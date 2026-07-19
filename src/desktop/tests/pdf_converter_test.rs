use fastmd::background::pdf_converter::PdfConversionJob;
use std::fs::File;
use tempfile::tempdir;
use std::sync::mpsc;

#[test]
fn test_pdf_conversion_should_convert() {
    let dir = tempdir().unwrap();
    let pdf_path = dir.path().join("test.pdf");
    let md_path = dir.path().join("test.md");
    
    // Create empty PDF file
    File::create(&pdf_path).unwrap();
    
    let job = PdfConversionJob::new(pdf_path.clone());
    assert_eq!(job.output_md, md_path);
    assert!(job.should_convert());
    
    // Create MD file
    File::create(&md_path).unwrap();
    
    // Wait a little so modified times could differ if we test modification
    // By default they might be the same or close. Since MD was created after, pdf > md is false.
    let job2 = PdfConversionJob::new(pdf_path.clone());
    assert!(!job2.should_convert());
}

#[tokio::test]
async fn test_pdf_conversion_execute_no_command() {
    let dir = tempdir().unwrap();
    let pdf_path = dir.path().join("test.pdf");
    File::create(&pdf_path).unwrap();
    let job = PdfConversionJob::new(pdf_path);
    
    let (tx, _rx) = mpsc::channel();
    let result = job.execute(None, tx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("No pdf_converter_command configured"));
}
