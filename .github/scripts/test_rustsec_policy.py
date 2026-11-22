import unittest

from rustsec_policy import classify_ignore, read_findings, removed_ignores, untracked_advisories


BEFORE = {
    "advisories": {"ignore": [
        {"id": "RUSTSEC-2026-0258", "reason": "keep unless fixed"},
        "RUSTSEC-2026-0542",
    ]},
    "licenses": {"allow": ["MIT"]},
}
FRESH = "RUSTSEC-2099-0001"
TRACKED = "RUSTSEC-2099-0002"


def advisory(id, severity="error"):
    return {"type": "diagnostic", "fields": {"severity": severity, "advisory": {"id": id}}}


def summary(errors):
    return {"type": "summary", "fields": {"advisories": {"errors": errors}}}


class RustSecScanTest(unittest.TestCase):
    def test_complete_scans_keep_warnings_and_deduplicate(self):
        self.assertEqual(read_findings([summary(0)], 0), [])
        self.assertEqual(
            read_findings([advisory(TRACKED), advisory(FRESH), advisory(FRESH, "warning"), summary(2)], 1),
            [FRESH, TRACKED],
        )

    def test_tracking_requires_exact_id_and_label(self):
        open_items = [
            {"title": TRACKED, "labels": [{"name": "rustsec"}]},
            {"title": FRESH + "0", "labels": [{"name": "rustsec"}]},
            {"title": FRESH, "labels": [{"name": "agent"}]},
        ]
        self.assertEqual(untracked_advisories([FRESH, TRACKED], open_items), [FRESH])

    def test_ignore_is_affected_if_either_probe_reports_it(self):
        clean = ([summary(0)], 0)
        warning = ([advisory(FRESH, "warning"), summary(0)], 0)
        unrelated = ([advisory(TRACKED), summary(1)], 1)
        self.assertEqual(classify_ignore(FRESH, clean, unrelated), "obsolete")
        self.assertEqual(classify_ignore(FRESH, clean, warning), "affected")
        self.assertEqual(classify_ignore(FRESH, warning, clean), "affected")
        with self.assertRaisesRegex(ValueError, "summary"):
            classify_ignore(FRESH, warning, ([], 1))

    def test_incomplete_or_invalid_scans_fail_closed(self):
        invalid_scans = [
            ([advisory(FRESH)], 1),
            ([advisory(FRESH), summary(0)], 1),
            ([summary(0)], 1),
            ([summary(0), summary(0)], 0),
            ([{"type": "log", "fields": {"level": "ERROR"}}, summary(0)], 0),
            ([{"type": "diagnostic", "fields": {"severity": "error"}}, summary(1)], 1),
            ([{"type": "diagnostic", "fields": {"code": "index-failure"}}, summary(0)], 0),
            ([advisory("invalid"), summary(1)], 1),
        ]
        for records, exit_code in invalid_scans:
            with self.subTest(records=records, exit_code=exit_code), self.assertRaises(ValueError):
                read_findings(records, exit_code)


class RustSecPolicyTest(unittest.TestCase):
    def test_removes_only_the_selected_obsolete_entry(self):
        after = {**BEFORE, "advisories": {"ignore": [BEFORE["advisories"]["ignore"][0]]}}
        self.assertEqual(
            removed_ignores(BEFORE, after, {"RUSTSEC-2026-0542": "obsolete"}, False),
            ["RUSTSEC-2026-0542"],
        )

    def test_affected_ignore_requires_a_dependency_change(self):
        after = {**BEFORE, "advisories": {"ignore": ["RUSTSEC-2026-0542"]}}
        with self.assertRaisesRegex(ValueError, "dependency change"):
            removed_ignores(BEFORE, after, {"RUSTSEC-2026-0258": "affected"}, False)

    def test_policy_change_is_rejected_even_with_an_obsolete_ignore(self):
        after = {**BEFORE, "advisories": {"ignore": []}, "licenses": {"allow": []}}
        with self.assertRaisesRegex(ValueError, "outside"):
            removed_ignores(BEFORE, after, {
                "RUSTSEC-2026-0258": "obsolete", "RUSTSEC-2026-0542": "obsolete",
            }, False)

    def test_untracked_candidate_required(self):
        after = {**BEFORE, "advisories": {"ignore": []}}
        with self.assertRaisesRegex(ValueError, "candidate"):
            removed_ignores(BEFORE, after, {"RUSTSEC-2026-0542": "obsolete"}, True)
