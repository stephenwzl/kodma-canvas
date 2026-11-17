//! Rebind presentation slots instead of repeatedly allocating AppKit views and KVO machinery.
//! Inactive slots never participate in action polling; owners and targets are rebound together.
#[derive(Default)]
pub(super) struct RowPool<T> {
    active: Vec<T>,
    spare: Vec<T>,
    high_water: usize,
}
impl<T> RowPool<T> {
    pub fn new() -> Self {
        Self { active: Vec::new(), spare: Vec::new(), high_water: 0 }
    }
    pub fn rebind<D>(
        &mut self,
        data: impl IntoIterator<Item = D>,
        mut create: impl FnMut(&D) -> T,
        mut bind: impl FnMut(&mut T, D),
        mut available: impl FnMut(&T, &D) -> bool,
    ) {
        self.spare.append(&mut self.active);
        for value in data {
            // A mouse-up still addressed to an old button must never dispatch its new owner.
            let slot = self.spare.iter().rposition(|row| available(row, &value));
            let mut row = slot.map(|index| self.spare.swap_remove(index)).unwrap_or_else(|| create(&value));
            bind(&mut row, value);
            self.active.push(row);
        }
        // Identity changes during a held mouse may require fresh slots. Keep at most one
        // high-water set of inactive slots; native row Drop detaches discarded views.
        self.high_water = self.high_water.max(self.active.len());
        self.spare.truncate(self.high_water);
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.active.iter()
    }
    pub fn inactive(&self) -> std::slice::Iter<'_, T> {
        self.spare.iter()
    }
    pub fn len(&self) -> usize {
        self.active.len()
    }
}

#[cfg(test)]
mod tests {
    use super::RowPool;
    use std::cell::Cell;

    #[test]
    fn native_row_pool_bounds_creation_across_home_close_and_reopen() {
        let count = Cell::new(0);
        let mut pool = RowPool::new();
        for _ in 0..100 {
            for ids in [vec![1, 2], vec![], vec![3], vec![4, 5]] {
                pool.rebind(
                    ids,
                    |_| {
                        count.set(count.get() + 1);
                        0
                    },
                    |row, uid| *row = uid,
                    |_, _| true,
                );
            }
        }
        assert_eq!(count.get(), 2, "view creation must stop at the largest visible set");
    }

    #[test]
    fn native_row_pool_excludes_hidden_actions_and_rebinds_identity() {
        let mut pool = RowPool::new();
        pool.rebind([1, 2], |_| 0, |row, uid| *row = uid, |_, _| true);
        pool.rebind([], |_| 0, |row, uid: u64| *row = uid, |_, _| true);
        assert_eq!(pool.iter().count(), 0, "hidden rows must not dispatch stale actions");
        pool.rebind([99], |_| 0, |row, uid| *row = uid, |_, _| true);
        assert_eq!(pool.iter().copied().collect::<Vec<_>>(), vec![99]);
        assert_eq!(pool.inactive().count(), 1);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn native_row_pool_does_not_retarget_a_tracking_button() {
        let mut pool = RowPool::new();
        pool.rebind([1], |_| (0, true), |row, uid| row.0 = uid, |_, _| true);
        pool.rebind([2], |_| (0, false), |row, uid| row.0 = uid, |row, uid| !row.1 || row.0 == *uid);
        assert_eq!(pool.iter().copied().collect::<Vec<_>>(), vec![(2, false)]);
        assert_eq!(pool.inactive().copied().collect::<Vec<_>>(), vec![(1, true)]);
    }

    #[test]
    fn native_row_pool_reuses_the_same_target_during_held_resize() {
        let count = Cell::new(0);
        let mut pool = RowPool::new();
        for _ in 0..100 {
            pool.rebind(
                [1, 2],
                |_| {
                    count.set(count.get() + 1);
                    0
                },
                |row, uid| *row = uid,
                |row, uid| row == uid,
            );
        }
        assert_eq!(count.get(), 2);
    }

    #[test]
    fn native_row_pool_bounds_unavailable_spares_during_identity_churn() {
        let mut pool = RowPool::new();
        for uid in 1..100 {
            pool.rebind([uid], |_| 0, |row, uid| *row = uid, |_, _| false);
            assert_eq!(pool.iter().copied().collect::<Vec<_>>(), vec![uid]);
            assert!(pool.inactive().count() <= 1);
        }
    }
}
