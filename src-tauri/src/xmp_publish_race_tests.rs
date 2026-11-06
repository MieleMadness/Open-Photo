//! Active Issue #90 publication race gate for the runtime reviewed-Apply path.
//! The deterministic external edit still occurs after comparison and before
//! candidate publication. It must survive; no original media is used.
#[cfg(windows)]
#[test]
fn external_edit_between_compare_and_successful_publish_must_not_be_lost() {
    crate::xmp_transactions::step3_tests::publication_race_gate();
}
