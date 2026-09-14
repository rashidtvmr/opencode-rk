"""Independent AUTO-003 RED contract for controller-owned worktree leases."""

from __future__ import annotations

import unittest

from tools import ralph_loop


class Auto003LeaseTests(unittest.TestCase):
    def test_acquire_records_owner_task_heartbeat_and_expiry(self) -> None:
        leases = ralph_loop.LeaseTable(max_leases=2)

        lease = leases.acquire(
            "AUTO-003",
            "worker-1",
            now=100.0,
            ttl_seconds=30.0,
        )

        self.assertEqual(lease.task_id, "AUTO-003")
        self.assertEqual(lease.owner, "worker-1")
        self.assertEqual(lease.acquired_at, 100.0)
        self.assertEqual(lease.heartbeat_at, 100.0)
        self.assertEqual(lease.expires_at, 130.0)
        self.assertEqual(leases.get("AUTO-003"), lease)

    def test_non_expired_foreign_lease_is_rejected_without_mutation(self) -> None:
        leases = ralph_loop.LeaseTable(max_leases=2)
        original = leases.acquire(
            "AUTO-003",
            "worker-1",
            now=100.0,
            ttl_seconds=30.0,
        )

        with self.assertRaises(ralph_loop.LeaseHeldError):
            leases.acquire(
                "AUTO-003",
                "worker-2",
                now=129.0,
                ttl_seconds=30.0,
            )

        self.assertEqual(leases.get("AUTO-003"), original)

    def test_expired_lease_is_reclaimed_by_new_owner(self) -> None:
        leases = ralph_loop.LeaseTable(max_leases=2)
        leases.acquire(
            "AUTO-003",
            "worker-1",
            now=100.0,
            ttl_seconds=30.0,
        )

        replacement = leases.acquire(
            "AUTO-003",
            "worker-2",
            now=131.0,
            ttl_seconds=20.0,
        )

        self.assertEqual(replacement.task_id, "AUTO-003")
        self.assertEqual(replacement.owner, "worker-2")
        self.assertEqual(replacement.acquired_at, 131.0)
        self.assertEqual(replacement.heartbeat_at, 131.0)
        self.assertEqual(replacement.expires_at, 151.0)
        self.assertEqual(leases.get("AUTO-003"), replacement)

    def test_only_owner_heartbeat_extends_expiry(self) -> None:
        leases = ralph_loop.LeaseTable(max_leases=2)
        original = leases.acquire(
            "AUTO-003",
            "worker-1",
            now=100.0,
            ttl_seconds=30.0,
        )

        with self.assertRaises(ralph_loop.LeaseOwnerError):
            leases.heartbeat(
                "AUTO-003",
                "worker-2",
                now=110.0,
                ttl_seconds=30.0,
            )
        self.assertEqual(leases.get("AUTO-003"), original)

        renewed = leases.heartbeat(
            "AUTO-003",
            "worker-1",
            now=110.0,
            ttl_seconds=30.0,
        )
        self.assertEqual(renewed.acquired_at, 100.0)
        self.assertEqual(renewed.heartbeat_at, 110.0)
        self.assertEqual(renewed.expires_at, 140.0)
        self.assertEqual(leases.get("AUTO-003"), renewed)

    def test_release_is_owner_only_and_ttl_and_capacity_fail_explicitly(self) -> None:
        leases = ralph_loop.LeaseTable(max_leases=1)

        with self.assertRaises(ralph_loop.InvalidLeaseTtlError):
            leases.acquire(
                "AUTO-invalid",
                "worker-1",
                now=100.0,
                ttl_seconds=0.0,
            )

        lease = leases.acquire(
            "AUTO-003",
            "worker-1",
            now=100.0,
            ttl_seconds=30.0,
        )
        with self.assertRaises(ralph_loop.LeaseCapacityError):
            leases.acquire(
                "AUTO-004",
                "worker-2",
                now=101.0,
                ttl_seconds=30.0,
            )
        self.assertEqual(leases.get("AUTO-003"), lease)

        with self.assertRaises(ralph_loop.LeaseOwnerError):
            leases.release("AUTO-003", "worker-2")
        self.assertEqual(leases.get("AUTO-003"), lease)

        leases.release("AUTO-003", "worker-1")
        self.assertIsNone(leases.get("AUTO-003"))


if __name__ == "__main__":
    unittest.main()
