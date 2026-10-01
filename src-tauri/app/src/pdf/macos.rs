//! macOS: `-[WKWebView printOperationWithPrintInfo:]` with a save-to-file job disposition,
//! run without panels. `runOperation` would block the main thread that WebKit needs to deliver
//! the rendered pages (and yields blank pages), so the modal variant is used with a delegate
//! that reports completion.

use super::{PageSetup, PrintDone};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, NSObject};
use objc2::{define_class, msg_send, sel, AllocAnyThread, DefinedClass};
use objc2_app_kit::{
    NSPrintInfo, NSPrintJobSavingURL, NSPrintOperation, NSPrintSaveJob, NSPrintingPaginationMode,
    NSWindow,
};
use objc2_foundation::{NSSize, NSString, NSURL};
use objc2_web_kit::WKWebView;
use std::cell::RefCell;
use std::ffi::c_void;
use std::path::Path;

/// Points per inch.
const PT: f64 = 72.0;

pub(super) struct DelegateIvars {
    done: RefCell<Option<PrintDone>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "BedrockEngineerPdfPrintDelegate"]
    #[ivars = DelegateIvars]
    pub(super) struct PrintDelegate;

    impl PrintDelegate {
        #[unsafe(method(printOperationDidRun:success:contextInfo:))]
        fn print_operation_did_run(
            &self,
            _operation: &NSPrintOperation,
            success: Bool,
            _context: *mut c_void,
        ) {
            if let Some(done) = self.ivars().done.borrow_mut().take() {
                done(if success.as_bool() {
                    Ok(())
                } else {
                    Err("The print operation failed".into())
                });
            }
            // Balance the reference leaked in `print_to_file`, via the autorelease pool so the
            // delegate outlives this call.
            let this: *const Self = self;
            if let Some(retained) = unsafe { Retained::from_raw(this as *mut Self) } {
                let _ = Retained::autorelease_ptr(retained);
            }
        }
    }
);

impl PrintDelegate {
    fn new(done: PrintDone) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateIvars {
            done: RefCell::new(Some(done)),
        });
        unsafe { msg_send![super(this), init] }
    }
}

pub(super) fn print_to_file(
    webview: tauri::webview::PlatformWebview,
    out: &Path,
    page: PageSetup,
    done: PrintDone,
) {
    unsafe {
        let wk: &WKWebView = &*(webview.inner() as *const WKWebView);
        let window: &NSWindow = &*(webview.ns_window() as *const NSWindow);

        let info = NSPrintInfo::new();
        info.setPaperSize(NSSize::new(page.width_in * PT, page.height_in * PT));
        let margin = page.margin_in * PT;
        info.setTopMargin(margin);
        info.setBottomMargin(margin);
        info.setLeftMargin(margin);
        info.setRightMargin(margin);
        info.setHorizontallyCentered(false);
        info.setVerticallyCentered(false);
        info.setHorizontalPagination(NSPrintingPaginationMode::Fit);
        info.setVerticalPagination(NSPrintingPaginationMode::Automatic);
        info.setJobDisposition(NSPrintSaveJob);
        let path = NSString::from_str(&out.to_string_lossy());
        let url = NSURL::fileURLWithPath(&path);
        let dict = info.dictionary();
        let _: () = msg_send![&*dict, setObject: &*url, forKey: NSPrintJobSavingURL];

        let operation = wk.printOperationWithPrintInfo(&info);
        operation.setShowsPrintPanel(false);
        operation.setShowsProgressPanel(false);
        // The operation's view needs a frame, or WebKit prints nothing.
        if let Some(view) = operation.view() {
            view.setFrame(wk.frame());
        }

        // Kept alive until `printOperationDidRun:` (NSPrintOperation doesn't retain it).
        let delegate = Retained::into_raw(PrintDelegate::new(done));
        operation.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
            window,
            Some(&*(delegate as *const AnyObject)),
            Some(sel!(printOperationDidRun:success:contextInfo:)),
            std::ptr::null_mut(),
        );
    }
}
