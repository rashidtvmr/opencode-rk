"""Task-driven slot allocation. No role has a fixed reservation or lane count."""
from __future__ import annotations

ROLES = ("implement", "test", "review", "integration_prepare", "repair")
WEIGHTS = {"implement": 5, "test": 7, "review": 9, "integration_prepare": 10, "repair": 11}


def allocate(demand: dict[str, int], capacity: int, *, integration_backlog: int = 0,
             max_implementations: int | None = None) -> dict[str, int]:
    if type(capacity) is not int or not 0 <= capacity <= 48:
        raise ValueError("capacity must be 0..48")
    if type(integration_backlog) is not int or integration_backlog < 0:
        raise ValueError("invalid backlog")
    if max_implementations is not None and (type(max_implementations) is not int or max_implementations < 0):
        raise ValueError("invalid implementation limit")
    if not isinstance(demand, dict) or set(demand) - set(ROLES) or any(type(v) is not int or v < 0 for v in demand.values()):
        raise ValueError("invalid demand")
    assigned = {role: 0 for role in ROLES}
    for _ in range(capacity):
        eligible = [r for r in ROLES if assigned[r] < demand.get(r, 0) and
                    (r != "implement" or max_implementations is None or assigned[r] < max_implementations)]
        if not eligible:
            break
        def score(role: str) -> tuple[int, int, int]:
            pressure = 3 * integration_backlog if role in {"review", "integration_prepare", "repair"} else 0
            return (WEIGHTS[role] + pressure - assigned[role] * 2,
                    demand[role] - assigned[role], -ROLES.index(role))
        assigned[max(eligible, key=score)] += 1
    return assigned
