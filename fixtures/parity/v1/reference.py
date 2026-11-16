"""Run selected golden cases against the Python reference without AWS or live data.

Usage: python fixtures/parity/v1/reference.py /path/to/finance-mgmt-backend-lite
Requires the reference backend's Python dependencies plus pytest/httpx. The
reference version and intentionally limited coverage are recorded in the docs.
"""

import json
import os
import re
import sys
import uuid
from pathlib import Path

if len(sys.argv) != 2:
    raise SystemExit("Pass the local Python backend checkout path")

backend = Path(sys.argv[1]).resolve()
if not (backend / "src" / "main.py").is_file():
    raise SystemExit("Not a Python backend checkout")

# Force the reference's module-level DB engine to use SQLite, even if a user has
# AWS-related environment variables configured. No network or live AWS access.
os.environ["DATABASE_URL"] = "sqlite://"
os.environ["ENVIRONMENT"] = "dev"
sys.path.insert(0, str(backend))

from fastapi.testclient import TestClient  # noqa: E402
from sqlalchemy import create_engine, event  # noqa: E402
from sqlalchemy.orm import sessionmaker  # noqa: E402
from sqlalchemy.pool import StaticPool  # noqa: E402

from src.db import get_db  # noqa: E402
from src.dependencies import get_current_user  # noqa: E402
from src.main import app  # noqa: E402
from src.models.base import Base  # noqa: E402
from src.models.user import User  # noqa: E402


def select(data, path):
    if not path:
        return data
    for part in path.split("."):
        data = data[int(part)] if isinstance(data, list) else data[part]
    return data


def resolve(value, saved):
    if isinstance(value, str):
        match = re.fullmatch(r"\$([\w-]+)(?:\.([\w.-]+))?", value)
        if match:
            return select(saved[match[1]], match[2] or "")
        return re.sub(r"\$([\w-]+)\.([\w.-]+)", lambda m: str(select(saved[m[1]], m[2])), value)
    if isinstance(value, list):
        return [resolve(item, saved) for item in value]
    if isinstance(value, dict):
        return {key: resolve(item, saved) for key, item in value.items()}
    return value


def run_case(case):
    engine = create_engine("sqlite://", connect_args={"check_same_thread": False}, poolclass=StaticPool)

    @event.listens_for(engine, "connect")
    def configure(connection, _record):
        connection.execute("PRAGMA foreign_keys=ON")

        def date_trunc(unit, value):
            if unit != "month":
                raise ValueError("Only month supported in reference fixtures")
            return f"{value[:7]}-01 00:00:00" if value else None

        connection.create_function("date_trunc", 2, date_trunc)

    Base.metadata.create_all(engine)
    db = sessionmaker(bind=engine, expire_on_commit=False)()
    user = User(id=uuid.uuid4(), cognito_sub=f"offline-fixture-{uuid.uuid4()}")
    db.add(user)
    db.commit()
    app.dependency_overrides[get_db] = lambda: db
    app.dependency_overrides[get_current_user] = lambda: user
    saved = {}
    try:
        with TestClient(app) as client:
            for step in case["steps"]:
                path = resolve(step["path"], saved)
                args = {"json": resolve(step["body"], saved)} if "body" in step else {}
                response = client.request(step["method"], path, **args)
                if response.status_code != step["status"]:
                    raise AssertionError(f"{case['id']}/{step['id']}: HTTP {response.status_code}: {response.text}")
                data = response.json()
                for field, expected in step.get("expect", {}).items():
                    actual = select(data, field)
                    expected = resolve(expected, saved)
                    if field == "postings":
                        actual = sorted(actual, key=lambda item: item["account_id"])
                        expected = sorted(expected, key=lambda item: item["account_id"])
                    if actual != expected:
                        raise AssertionError(f"{case['id']}/{step['id']}.{field}: {actual!r} != {expected!r}")
                for assertion in step.get("expect_row", []):
                    rows = select(data, assertion["collection"])
                    where = resolve(assertion["where"], saved)
                    matches = [row for row in rows if all(row[key] == value for key, value in where.items())]
                    if len(matches) != 1:
                        raise AssertionError(f"{case['id']}/{step['id']}: expected one row for {where}, found {len(matches)}")
                    actual = select(matches[0], assertion["field"])
                    if actual != assertion["equals"]:
                        raise AssertionError(f"{case['id']}/{step['id']}: {actual!r} != {assertion['equals']!r}")
                saved[step["id"]] = data
    finally:
        app.dependency_overrides.clear()
        db.close()
        engine.dispose()


def main():
    cases = json.loads(Path(__file__).with_name("cases.json").read_text())
    if cases["version"] != 1:
        raise SystemExit("Unsupported fixture version")
    for case in cases["cases"]:
        run_case(case)
        print(f"PASS {case['id']} ({len(case['steps'])} steps)")


if __name__ == "__main__":
    main()
