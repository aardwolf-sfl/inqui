use std::cell::RefCell;

use crate::query::QueryId;

#[derive(Debug, Default)]
pub(crate) struct QueryStack {
    active: RefCell<Vec<QueryId>>,
}

impl QueryStack {
    pub fn push(&self, query_id: QueryId) -> Result<ActiveQueryGuard<'_>, Cycle> {
        let mut active = self.active.borrow_mut();

        if let Some(cycle_start) = active
            .iter()
            .copied()
            .enumerate()
            .rev()
            .find_map(|(i, on_stack)| (on_stack == query_id).then(|| i))
        {
            let mut cycle = active[cycle_start..].to_vec();
            cycle.push(query_id);

            return Err(Cycle { cycle });
        }

        active.push(query_id);
        let pop_at = active.len();

        Ok(ActiveQueryGuard {
            query_stack: self,
            pop_at,
        })
    }
}

pub(crate) struct ActiveQueryGuard<'q> {
    query_stack: &'q QueryStack,
    pop_at: usize,
}

impl Drop for ActiveQueryGuard<'_> {
    fn drop(&mut self) {
        let mut active = self.query_stack.active.borrow_mut();
        assert_eq!(active.len(), self.pop_at);
        active.pop();
    }
}

#[derive(Debug)]
pub struct Cycle {
    cycle: Vec<QueryId>,
}

impl Cycle {
    pub fn cycle(&self) -> &[QueryId] {
        self.cycle.as_slice()
    }
}
