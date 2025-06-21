use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    errors::{ExecutionError, QueryError, TransactionError},
    index::plan::{index_join_plan::IndexJoinPlan, index_select_plan::IndexSelectPlan},
    metadata::{index_manager::IndexInfo, MetadataManager},
    multibuffer::multibuffer_product_plan::MultiBufferProductPlan,
    parser::predicate::Predicate,
    plan::{select_plan::SelectPlan, table_plan::TablePlan, Plan, PlanControl},
    record::schema::Schema,
    tx::transaction::Transaction,
};

pub struct TablePlanner {
    table_plan: TablePlan,
    predicate: Option<Predicate>,
    tx: Arc<Mutex<Transaction>>,
    index_info_map: HashMap<String, IndexInfo>,
}

impl TablePlanner {
    pub(crate) fn new(
        table_name: &str,
        predicate: &Option<Predicate>,
        tx: Arc<Mutex<Transaction>>,
        metadata_manager: Arc<Mutex<MetadataManager>>,
    ) -> Result<Self, ExecutionError> {
        {
            let md = metadata_manager.lock().unwrap();
            if md.get_layout(table_name, tx.clone())?.is_none() {
                return Err(QueryError::TableNotFound(table_name.to_string()).into());
            }
        }
        let table_plan = TablePlan::new(tx.clone(), table_name, metadata_manager.clone())?;
        let index_info_map = metadata_manager
            .lock()
            .unwrap()
            .get_index_info(table_name, tx.clone())?;

        Ok(Self {
            table_plan,
            predicate: predicate.clone(),
            tx,
            index_info_map,
        })
    }

    pub(crate) fn make_select_plan(&self) -> Result<Plan, TransactionError> {
        if let Some(predicate) = &self.predicate {
            for field_name in self.index_info_map.keys() {
                match predicate.equates_with_constant(field_name) {
                    Some(value) => {
                        let index_info = self.index_info_map.get(field_name).unwrap();
                        let index_select_plan =
                            IndexSelectPlan::new(self.table_plan.clone(), index_info, &value);
                        return Ok(self.add_select_predicates(
                            Plan::from(index_select_plan),
                            predicate.clone(),
                        ));
                    }
                    None => {}
                }
            }
            return Ok(
                self.add_select_predicates(Plan::from(self.table_plan.clone()), predicate.clone())
            );
        }
        Ok(Plan::from(self.table_plan.clone()))
    }

    pub(crate) fn make_join_plan(
        &self,
        current_plan: Plan,
    ) -> Result<Option<Plan>, TransactionError> {
        if let Some(predicate) = self.predicate.clone() {
            let join_sub_predicates =
                predicate.join_sub_predicates(self.table_plan.schema(), current_plan.schema());
            if join_sub_predicates.is_none() {
                return Ok(None);
            }

            if let Some(plan) = self.make_index_join(current_plan.clone())? {
                return Ok(Some(plan));
            }
            return Ok(Some(self.make_product_join(current_plan.clone())?));
        } else {
            Ok(None)
        }
    }

    pub(crate) fn make_product_plan(&self, current_plan: Plan) -> Result<Plan, TransactionError> {
        let table_plan = Plan::from(self.table_plan.clone());
        let plan = if let Some(predicate) = self.predicate.clone() {
            self.add_select_predicates(table_plan, predicate)
        } else {
            table_plan
        };
        Ok(Plan::from(MultiBufferProductPlan::new(
            self.tx.clone(),
            current_plan,
            plan,
        )))
    }

    fn make_index_join(&self, current_plan: Plan) -> Result<Option<Plan>, TransactionError> {
        if let Some(predicate) = self.predicate.clone() {
            for field_name in self.index_info_map.keys() {
                let outer_field_name = predicate.equates_with_field(field_name);
                if outer_field_name.is_none() {
                    continue;
                }
                let outer_field_name = outer_field_name.unwrap();
                if !current_plan.schema().has_field(&outer_field_name) {
                    continue;
                }
                let index_info = self.index_info_map.get(field_name).unwrap();
                let plan = IndexJoinPlan::new(
                    current_plan.clone(),
                    Plan::from(self.table_plan.clone()),
                    index_info.clone(),
                    outer_field_name,
                );
                let plan = self.add_select_predicates(Plan::from(plan), predicate);
                let plan = self.add_join_predicates(plan, current_plan.schema());
                return Ok(Some(plan));
            }
            Ok(None)
        } else {
            Ok(None)
        }
    }

    fn make_product_join(&self, current_plan: Plan) -> Result<Plan, TransactionError> {
        let plan = self.make_product_plan(current_plan.clone())?;
        Ok(self.add_join_predicates(plan, current_plan.schema()))
    }

    fn add_select_predicates(&self, plan: Plan, predicate: Predicate) -> Plan {
        if let Some(predicate) = predicate.select_sub_predicates(plan.schema()) {
            Plan::from(SelectPlan::new(plan, predicate))
        } else {
            Plan::from(plan)
        }
    }

    fn add_join_predicates(&self, plan: Plan, current_schema: &Schema) -> Plan {
        if let Some(predicate) = self.predicate.clone() {
            if let Some(predicate) =
                predicate.join_sub_predicates(current_schema, self.table_plan.schema())
            {
                Plan::from(SelectPlan::new(plan, predicate))
            } else {
                Plan::from(plan)
            }
        } else {
            Plan::from(plan)
        }
    }
}
