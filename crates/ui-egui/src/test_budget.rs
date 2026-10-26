//! Memory budgets in tests: each test app's own cache budget, and the tests that change the
//! process-wide budget ([`lightcraft_engine::memory::budget`]).
//!
//! **Each test app's caches** (the session's [`MediaCache`](lightcraft_engine::media::MediaCache)
//! and the renderer's stage caches) are sized for its test's share of the machine, not for the
//! whole machine as an app's are: libtest runs several tests at once in one process, and each
//! took the full share, so the run held that many times over. [`LightcraftApp::new`] calls
//! [`cap_instance`]: the app takes what it would on a machine with
//! [`lightcraft_testkit::memory::per_test_bytes`] of RAM (issue #3066). The process-wide budget,
//! the decode, export and GPU gates and the demo scene cache stay as they are.
//!
//! **Changing the process-wide budget.** It is one number for the whole test process: the decode,
//! export and GPU gates, the caches of sessions made outside a test app and the demo scene cache
//! size from it, and other tests run at the same time. A test that changes it (through
//! `app.memoryBudget` or Settings ▸ Performance) holds a [`BudgetGuard`] for as long as the change
//! lasts. Tests that change the budget then run one at a time, so one can't change it under
//! another's assertion. The budget goes back to exactly what it was before, also when the test
//! fails: putting back the value found, not `default_budget()`, keeps a budget set from outside
//! (`LIGHTCRAFT_MEMORY_MB`, a test harness) (issue #3065).
//!
//! Tests that only read the budget don't take the lock; keep the change short.
//! The lock is per process, so per test binary, which is all the budget is shared by.

use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use lightcraft_engine::memory;

use crate::LightcraftApp;

/// The least budget a test app gets, as [`memory::default_budget`] gives the smallest machine.
const MIN_INSTANCE_BUDGET: usize = 256 << 20;
/// The most budget a test app gets, as [`memory::default_budget`] gives the largest machine.
const MAX_INSTANCE_BUDGET: usize = 3 << 29;

/// Size `app`'s own caches for its test's share of the machine (issue #3066): the session's cache
/// share and the renderer's stage budget come from [`instance_budget`] instead of the process-wide
/// budget. A test that sets either itself afterwards still can.
pub(crate) fn cap_instance(mut app: LightcraftApp) -> LightcraftApp {
    let budget = instance_budget();
    app.session.media.set_budget(memory::cache_share(budget));
    app.renderer.stage_budget_override = Some(budget / crate::render::STAGE_BUDGET_SHARE);
    app
}

/// The memory budget a test app sizes its own caches from: [`budget_for_share`] of this test's
/// share of the machine, read once per test process so that every test in a run gets the same.
pub(crate) fn instance_budget() -> usize {
    static PER_TEST: OnceLock<u64> = OnceLock::new();
    let per_test = *PER_TEST.get_or_init(lightcraft_testkit::memory::per_test_bytes);
    budget_for_share(per_test, memory::budget())
}

/// What [`memory::default_budget`] gives a machine with `per_test` bytes of RAM (a quarter, from
/// 256 MiB to 1.5 GiB), and never more than the process-wide budget `process`, so a lower
/// `LIGHTCRAFT_MEMORY_MB` still wins.
fn budget_for_share(per_test: u64, process: usize) -> usize {
    let quarter = usize::try_from(per_test / 4).unwrap_or(usize::MAX);
    quarter.clamp(MIN_INSTANCE_BUDGET, MAX_INSTANCE_BUDGET).min(process)
}

/// Taken by every test that changes the process-wide budget.
static CHANGING_BUDGET: Mutex<()> = Mutex::new(());

/// Held while a test changes the process-wide memory budget: the other tests that change it wait,
/// and the budget is put back as it was found when the guard drops.
pub(crate) struct BudgetGuard {
    // fields drop in order: the budget is put back before the next test may take its turn
    _restore: Restore,
    _turn: MutexGuard<'static, ()>,
}

impl BudgetGuard {
    /// Wait for the other tests that change the budget, then remember the budget as it is now.
    pub(crate) fn hold() -> BudgetGuard {
        // poisoned: a test failed holding it, and its guard put the budget back on unwind
        let turn = CHANGING_BUDGET.lock().unwrap_or_else(PoisonError::into_inner);
        BudgetGuard { _restore: Restore::new(), _turn: turn }
    }
}

/// Puts the process-wide budget back to what it was when made.
struct Restore(usize);

impl Restore {
    fn new() -> Restore {
        Restore(memory::budget())
    }
}

impl Drop for Restore {
    fn drop(&mut self) {
        // doesn't panic; the value came from `budget()`, so it's within what `set_budget` keeps
        memory::set_budget(self.0);
    }
}

mod tests {
    use super::*;

    const GIB: u64 = 1 << 30;

    // A test app takes what an app would on a machine with its test's share of memory: a quarter,
    // in the app's range, never more than the process-wide budget (issue #3066)
    #[test]
    fn a_test_apps_budget_is_a_quarter_of_its_share() {
        let process = 3 << 29;
        // 16 GiB available, 4 tests at once: 4 GiB each, 1 GiB of budget
        assert_eq!(budget_for_share(4 * GIB, process), 1 << 30);
        assert_eq!(budget_for_share(2 * GIB, process), 512 << 20);
        assert_eq!(budget_for_share(lightcraft_testkit::memory::MIN_PER_TEST, process), MIN_INSTANCE_BUDGET);
        assert_eq!(budget_for_share(0, process), MIN_INSTANCE_BUDGET, "never below the app's least");
        assert_eq!(budget_for_share(u64::MAX, process), MAX_INSTANCE_BUDGET, "never above the app's most");
        assert_eq!(budget_for_share(4 * GIB, 384 << 20), 384 << 20, "a lower LIGHTCRAFT_MEMORY_MB wins");
        assert_eq!(budget_for_share(4 * GIB, 8 << 30), 1 << 30, "a higher one doesn't");
    }

    // Given a test app, its session's caches and its renderer's stage caches take its test's
    // share, not the whole machine's (issue #3066)
    #[test]
    fn a_test_app_is_sized_for_its_tests_share() {
        // the process-wide budget, which the cap never exceeds, holds still meanwhile
        let _turn = BudgetGuard::hold();
        let h = crate::headless::Headless::new(LightcraftApp::new(lightcraft_engine::Session::with_demo(), Default::default()), [800.0, 600.0], 1.0);
        let budget = instance_budget();
        // e.g. 16 GiB available and 4 tests at once: 1 GiB, not the 1.5 GiB an app takes
        assert!(budget <= memory::budget());
        assert_eq!(h.app.session.media.budget(), memory::cache_share(budget));
        assert_eq!(h.app.renderer.stage_budget(), budget / crate::render::STAGE_BUDGET_SHARE);
        assert_eq!(h.app.renderer.memory()["stageCaches"]["budgetBytes"], budget / crate::render::STAGE_BUDGET_SHARE, "and reports it");
    }

    // The budget is put back as it was found, not to the default, also when the test fails
    // (issue #3065)
    #[test]
    fn the_budget_is_put_back_as_it_was_found() {
        let _turn = BudgetGuard::hold();
        // a budget set from outside the test, unlike the default
        let outside = if memory::budget() == 700 << 20 { 600 << 20 } else { 700 << 20 };
        memory::set_budget(outside);

        {
            let _r = Restore::new();
            memory::set_budget(1024 << 20);
            assert_eq!(memory::budget(), 1024 << 20);
        }
        assert_eq!(memory::budget(), outside, "the budget found, not the default");

        let failed = std::panic::catch_unwind(|| {
            let _r = Restore::new();
            memory::set_budget(96 << 20);
            panic!("an assertion fails while the budget is changed");
        });
        assert!(failed.is_err());
        assert_eq!(memory::budget(), outside, "put back on unwind too");
    }

    // A test that fails holding the lock doesn't make the next one fail
    #[test]
    fn a_failed_test_does_not_block_the_next() {
        let failed = std::thread::spawn(|| {
            let _g = BudgetGuard::hold();
            panic!("an assertion fails while holding the turn");
        })
        .join();
        assert!(failed.is_err());
        let _g = BudgetGuard::hold();
    }
}
