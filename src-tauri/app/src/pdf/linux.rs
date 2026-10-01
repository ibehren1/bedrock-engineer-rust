//! Linux: a WebKitGTK `PrintOperation` sent, without its dialog, to GTK's "Print to File"
//! printer writing PDF.

use super::{PageSetup, PrintDone};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use webkit2gtk::PrintOperationExt;

pub(super) fn print_to_file(
    webview: tauri::webview::PlatformWebview,
    out: &Path,
    page: PageSetup,
    done: PrintDone,
) {
    let settings = gtk::PrintSettings::new();
    settings.set_printer("Print to File");
    settings.set("output-file-format", Some("pdf"));
    let uri = tauri::Url::from_file_path(out)
        .map(String::from)
        .unwrap_or_else(|()| format!("file://{}", out.to_string_lossy()));
    settings.set("output-uri", Some(&uri));

    let setup = gtk::PageSetup::new();
    let paper = gtk::PaperSize::new_custom(
        "bedrock-engineer-letter",
        "Letter",
        page.width_in,
        page.height_in,
        gtk::Unit::Inch,
    );
    setup.set_paper_size(&paper);
    setup.set_orientation(gtk::PageOrientation::Portrait);
    setup.set_top_margin(page.margin_in, gtk::Unit::Inch);
    setup.set_bottom_margin(page.margin_in, gtk::Unit::Inch);
    setup.set_left_margin(page.margin_in, gtk::Unit::Inch);
    setup.set_right_margin(page.margin_in, gtk::Unit::Inch);
    settings.set_paper_size(&paper);

    let operation = webkit2gtk::PrintOperation::new(&webview.inner());
    operation.set_print_settings(&settings);
    operation.set_page_setup(&setup);

    // `failed` is followed by `finished`; report whichever comes first.
    let done = Rc::new(RefCell::new(Some(done)));
    let on_failed = done.clone();
    operation.connect_failed(move |_, error| {
        if let Some(done) = on_failed.borrow_mut().take() {
            done(Err(error.to_string()));
        }
    });
    operation.connect_finished(move |_| {
        if let Some(done) = done.borrow_mut().take() {
            done(Ok(()));
        }
    });
    operation.print();
}
