"""RED for COORD-007: provider/RAM/validation/retry budgets."""
import unittest

from tools.completion_budgets import BudgetEnforcer, ProviderBudget, ProviderLimits, should_retry


class BudgetRedTests(unittest.TestCase):
    def test_ram_oversubscribe_refused(self):
        assert False, "RED: admission must refuse when available < stopAdmittingBelowAvailableMiB"

    def test_retry_budget_exhausted_stops_lane(self):
        assert False, "RED: repairable failures must stop after maxAttemptsPerTask=3"

    def test_provider_quota_enforced(self):
        assert False, "RED: provider concurrency/rate/cost/Retry-After must refuse over-quota acquire/spend"


if __name__ == "__main__":
    unittest.main()
