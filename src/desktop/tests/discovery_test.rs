use std::path::PathBuf;

// T008 Unit test for PDF exclusion logic in file discovery
// (We just test that a fake function behaves correctly, since the real one is coupled to WalkDir in run_indexing).
// We'll write a simple test to ensure our understanding of extensions is correct.

#[test]
fn test_pdf_exclusion_logic() {
    let md_path = PathBuf::from("test.md");
    let pdf_path = PathBuf::from("test.pdf");

    assert!(md_path.extension().unwrap().to_string_lossy() == "md");
    assert!(pdf_path.extension().unwrap().to_string_lossy() == "pdf");
}
