"""Waitlist database for Concerto preview access and Symphonia interest."""

import logging
import os

import resend
from psycopg2 import connect

log = logging.getLogger(__name__)

DATABASE_URL = os.environ.get("WEBSITE_DB_URL")
RESEND_API_KEY = os.environ.get("RESEND_API_KEY")
NOTIFY_EMAIL = "jack@loopflow.studio"

if RESEND_API_KEY:
    resend.api_key = RESEND_API_KEY

SCHEMA = """
CREATE TABLE IF NOT EXISTS waitlist (
    id SERIAL PRIMARY KEY,
    email TEXT NOT NULL,
    product TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(email, product)
);
"""


def init_db() -> None:
    if not DATABASE_URL:
        log.info("Waitlist database disabled (WEBSITE_DB_URL not set)")
        return
    try:
        with connect(DATABASE_URL) as conn, conn.cursor() as cur:
            cur.execute(SCHEMA)
    except Exception as error:
        log.warning("Failed to initialize waitlist database: %s", error)


def add_to_waitlist(email: str, product: str) -> bool | None:
    """Add an email to the waitlist. Returns True if new, False if duplicate, None if no DB."""
    if not DATABASE_URL:
        return None
    try:
        with connect(DATABASE_URL) as conn, conn.cursor() as cur:
            cur.execute(
                "INSERT INTO waitlist (email, product) VALUES (%s, %s) ON CONFLICT (email, product) DO NOTHING RETURNING id",
                (email, product),
            )
            is_new = cur.fetchone() is not None
    except Exception as error:
        log.warning("Failed to write to waitlist: %s", error)
        return None

    if is_new:
        _notify_signup(email, product)
    return is_new


def _notify_signup(email: str, product: str) -> None:
    if not RESEND_API_KEY:
        return
    try:
        resend.Emails.send(
            {
                "from": "Loopflow <notifications@loopflow.studio>",
                "to": NOTIFY_EMAIL,
                "subject": f"Waitlist signup: {product}",
                "text": f"{email} signed up for {product}",
            }
        )
    except Exception as error:
        log.warning("Failed to send waitlist notification: %s", error)
