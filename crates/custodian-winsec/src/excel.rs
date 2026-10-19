//! Fixed Excel object-model operations for a dedicated task-owned instance.
//! Call only in a supervised worker. COM calls can block; never call on the UI.
#[cfg(windows)]
pub use native::ExcelSession;

/// Exact version boundary shared by admission and the native COM worker.
/// Changes require real-application qualification; never admit a version range.
pub fn version_is_supported(version: [u16; 4]) -> bool {
    matches!(version, [16, 0, 20430, 20146] | [16, 0, 20430, 20182])
}

#[cfg(test)]
mod version_tests {
    #[test]
    fn qualification_does_not_admit_adjacent_patches_or_other_office_versions() {
        assert!(super::version_is_supported([16, 0, 20430, 20146]));
        assert!(super::version_is_supported([16, 0, 20430, 20182]));
        for version in [
            [16, 0, 20430, 20147],
            [16, 0, 20430, 20181],
            [16, 0, 20430, 20183],
            [16, 0, 20431, 20182],
            [16, 1, 20430, 20182],
            [17, 0, 20430, 20182],
        ] {
            assert!(!super::version_is_supported(version));
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CellValue {
    Empty,
    Text {
        value: String,
    },
    Number {
        value: f64,
    },
    Sum {
        first_row: u16,
        first_col: u16,
        last_row: u16,
        last_col: u16,
    },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub row: u16,
    pub col: u16,
    pub value: CellValue,
    pub calculated: Option<f64>,
    pub text_format: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub sheet: String,
    pub cells: Vec<Cell>,
}

fn address(row: u16, col: u16) -> Option<String> {
    if !(1..=16).contains(&row) || !(1..=16).contains(&col) {
        return None;
    }
    Some(format!("{}{row}", char::from(b'A' + col as u8 - 1)))
}
fn sum_formula(first_row: u16, first_col: u16, last_row: u16, last_col: u16) -> Option<String> {
    if first_row > last_row || first_col > last_col {
        return None;
    }
    Some(format!(
        "=SUM({}:{})",
        address(first_row, first_col)?,
        address(last_row, last_col)?
    ))
}
pub fn parse_sum(formula: &str) -> Option<CellValue> {
    let body = formula.strip_prefix("=SUM(")?.strip_suffix(')')?;
    let (first, last) = body.split_once(':')?;
    let cell = |s: &str| -> Option<(u16, u16)> {
        let col = *s.as_bytes().first()?;
        if !(b'A'..=b'P').contains(&col) {
            return None;
        }
        let row = s[1..].parse::<u16>().ok()?;
        if address(row, (col - b'A' + 1) as u16)?.as_str() != s {
            return None;
        }
        Some((row, (col - b'A' + 1) as u16))
    };
    let (first_row, first_col) = cell(first)?;
    let (last_row, last_col) = cell(last)?;
    sum_formula(first_row, first_col, last_row, last_col)?;
    Some(CellValue::Sum {
        first_row,
        first_col,
        last_row,
        last_col,
    })
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod native {
    use super::*;
    use std::{
        io,
        marker::PhantomData,
        path::{Path, PathBuf},
        rc::Rc,
    };
    use windows::{
        core::{BSTR, GUID, PCWSTR},
        Win32::System::{
            Com::{
                CLSIDFromProgID, CoCreateInstance, CoInitializeEx, CoUninitialize, IDispatch,
                CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED, DISPATCH_FLAGS, DISPATCH_METHOD,
                DISPATCH_PROPERTYGET, DISPATCH_PROPERTYPUT, DISPPARAMS, EXCEPINFO,
            },
            Ole::{
                SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound,
                SafeArrayGetVartype,
            },
            Variant::{
                VARIANT, VT_ARRAY, VT_BSTR, VT_EMPTY, VT_I2, VT_I4, VT_NULL, VT_R4, VT_R8,
                VT_VARIANT,
            },
        },
    };
    fn invalid() -> io::Error {
        io::Error::other("Qualified Excel operation failed; preserve task recovery state")
    }
    struct Apartment(PhantomData<Rc<()>>);
    impl Apartment {
        fn start() -> io::Result<Self> {
            // SAFETY: initializes this worker thread's COM apartment; non-Send
            // marker prevents moving COM interfaces to a different thread.
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                .ok()
                .map_err(|_| invalid())?;
            Ok(Self(PhantomData))
        }
    }
    impl Drop for Apartment {
        fn drop(&mut self) {
            // SAFETY: balanced successful initialization on the same thread.
            unsafe {
                CoUninitialize();
            }
        }
    }
    struct Dispatch(
        IDispatch,
        std::cell::RefCell<std::collections::BTreeMap<&'static str, i32>>,
    );
    fn values(raw: VARIANT, rows: i32, columns: i32) -> io::Result<Vec<VARIANT>> {
        if !(1..=16).contains(&rows) || !(1..=16).contains(&columns) {
            return Err(invalid());
        }
        if rows == 1 && columns == 1 {
            return Ok(vec![raw]);
        }
        if raw.vt().0 != VT_ARRAY.0 | VT_VARIANT.0 {
            return Err(invalid());
        }
        // SAFETY: exact non-byref VARIANT array tag checked above. The owner stays
        // alive through all accesses; each element is copied into a fresh owned
        // VARIANT by OleAut32. Require exact bounded two-dimensional Excel shape.
        unsafe {
            let array = raw.Anonymous.Anonymous.Anonymous.parray;
            if array.is_null()
                || SafeArrayGetDim(array) != 2
                || SafeArrayGetVartype(array).map_err(|_| invalid())? != VT_VARIANT
                || SafeArrayGetLBound(array, 1).map_err(|_| invalid())? != 1
                || SafeArrayGetLBound(array, 2).map_err(|_| invalid())? != 1
                || SafeArrayGetUBound(array, 1).map_err(|_| invalid())? != rows
                || SafeArrayGetUBound(array, 2).map_err(|_| invalid())? != columns
            {
                return Err(invalid());
            }
            let mut result = Vec::with_capacity((rows * columns) as usize);
            for row in 1..=rows {
                for col in 1..=columns {
                    let mut value = VARIANT::default();
                    SafeArrayGetElement(
                        array,
                        [row, col].as_ptr(),
                        (&mut value as *mut VARIANT).cast(),
                    )
                    .map_err(|_| invalid())?;
                    result.push(value);
                }
            }
            Ok(result)
        }
    }
    impl Dispatch {
        fn invoke(
            &self,
            name: &'static str,
            flags: DISPATCH_FLAGS,
            mut args: Vec<VARIANT>,
        ) -> io::Result<VARIANT> {
            let cached = self.1.borrow().get(name).copied();
            let id = if let Some(id) = cached {
                id
            } else {
                let text = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
                let mut id = 0;
                // SAFETY: fixed native member name on this live dispatch object.
                unsafe {
                    self.0
                        .GetIDsOfNames(&GUID::zeroed(), &PCWSTR(text.as_ptr()), 1, 0x409, &mut id)
                }
                .map_err(|_| invalid())?;
                self.1.borrow_mut().insert(name, id);
                id
            };
            args.reverse();
            let mut named = -3i32;
            let put = flags == DISPATCH_PROPERTYPUT;
            let params = DISPPARAMS {
                rgvarg: if args.is_empty() {
                    std::ptr::null_mut()
                } else {
                    args.as_mut_ptr()
                },
                rgdispidNamedArgs: if put {
                    &mut named
                } else {
                    std::ptr::null_mut()
                },
                cArgs: args.len() as u32,
                cNamedArgs: u32::from(put),
            };
            let mut output = VARIANT::default();
            let mut exception = EXCEPINFO::default();
            let mut argument = 0;
            // SAFETY: argument VARIANTs and named-put ID live through Invoke;
            // all results/exception BSTRs are owned RAII values. Untrusted COM
            // error strings never cross the adapter's public error boundary.
            unsafe {
                self.0.Invoke(
                    id,
                    &GUID::zeroed(),
                    0x409,
                    flags,
                    &params,
                    Some(&mut output),
                    Some(&mut exception),
                    Some(&mut argument),
                )
            }
            .map_err(|_| invalid())?;
            Ok(output)
        }
        fn get(&self, name: &'static str) -> io::Result<VARIANT> {
            self.invoke(name, DISPATCH_PROPERTYGET, vec![])
        }
        fn object(&self, name: &'static str) -> io::Result<Self> {
            Self::from(self.get(name)?)
        }
        fn item(&self, index: i32) -> io::Result<Self> {
            Self::from(self.invoke(
                "Item",
                DISPATCH_PROPERTYGET | DISPATCH_METHOD,
                vec![index.into()],
            )?)
        }
        fn from(value: VARIANT) -> io::Result<Self> {
            Ok(Self(
                IDispatch::try_from(&value).map_err(|_| invalid())?,
                Default::default(),
            ))
        }
        fn set(&self, name: &'static str, value: VARIANT) -> io::Result<()> {
            self.invoke(name, DISPATCH_PROPERTYPUT, vec![value])
                .map(|_| ())
        }
        fn method(&self, name: &'static str, args: Vec<VARIANT>) -> io::Result<()> {
            self.invoke(name, DISPATCH_METHOD, args).map(|_| ())
        }
        fn number(&self, name: &'static str) -> io::Result<i32> {
            i32::try_from(&self.get(name)?).map_err(|_| invalid())
        }
        fn boolean(&self, name: &'static str) -> io::Result<bool> {
            bool::try_from(&self.get(name)?).map_err(|_| invalid())
        }
        fn text(&self, name: &'static str) -> io::Result<String> {
            Ok(BSTR::try_from(&self.get(name)?)
                .map_err(|_| invalid())?
                .to_string())
        }
    }

    pub struct ExcelSession {
        sheet: Option<Dispatch>,
        book: Option<Dispatch>,
        app: Option<Dispatch>,
        identity: crate::apps::ProcessIdentity,
        source: Option<PathBuf>,
        current: Option<PathBuf>,
        sheet_name: Option<String>,
        _apartment: Apartment,
    }
    impl ExcelSession {
        pub fn start(existing: &[u32]) -> io::Result<Self> {
            let apartment = Apartment::start()?;
            let name = "Excel.Application\0".encode_utf16().collect::<Vec<_>>();
            // SAFETY: fixed registered Excel class only, no model-provided CLSID.
            let class = unsafe { CLSIDFromProgID(PCWSTR(name.as_ptr())) }.map_err(|_| invalid())?;
            // SAFETY: create a local server IDispatch on this initialized STA.
            let app = Dispatch(
                unsafe { CoCreateInstance(&class, None, CLSCTX_LOCAL_SERVER) }
                    .map_err(|_| invalid())?,
                Default::default(),
            );
            let hwnd = app.number("Hwnd")? as u32;
            let mut pid = 0;
            // SAFETY: native metadata query only on the returned application's HWND.
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                    hwnd as usize as _,
                    &mut pid,
                );
            }
            let identity = crate::apps::process_identity(pid).ok_or_else(invalid)?;
            if existing.contains(&pid)
                || !identity
                    .executable
                    .to_ascii_lowercase()
                    .ends_with("\\excel.exe")
                || !super::version_is_supported(crate::app_instance::file_version(Path::new(
                    &identity.executable,
                ))?)
                || app.object("Workbooks")?.number("Count")? != 0
            {
                return Err(invalid());
            }
            app.set("AutomationSecurity", 3i32.into())?;
            if app.number("AutomationSecurity")? != 3 {
                return Err(invalid());
            }
            app.set("EnableEvents", false.into())?;
            app.set("Visible", false.into())?;
            Ok(Self {
                sheet: None,
                book: None,
                app: Some(app),
                identity,
                source: None,
                current: None,
                sheet_name: None,
                _apartment: apartment,
            })
        }
        pub fn identity(&self) -> &crate::apps::ProcessIdentity {
            &self.identity
        }
        pub fn window(&self) -> io::Result<i64> {
            Ok(self.app.as_ref().ok_or_else(invalid)?.number("Hwnd")? as u32 as i64)
        }
        fn current(&self) -> io::Result<()> {
            let app = self.app.as_ref().ok_or_else(invalid)?;
            if !crate::apps::process_is_running(self.identity.pid, self.identity.creation_time)?
                || app.object("Workbooks")?.number("Count")? != 1
                || app.boolean("Visible")?
                || !app.boolean("Interactive")?
            {
                return Err(invalid());
            }
            let book = self.book.as_ref().ok_or_else(invalid)?;
            let sheet = self.sheet.as_ref().ok_or_else(invalid)?;
            if book.object("Worksheets")?.number("Count")? != 1
                || book.object("Windows")?.number("Count")? != 1
                || !book.text("FullName")?.eq_ignore_ascii_case(
                    &self.current.as_ref().ok_or_else(invalid)?.to_string_lossy(),
                )
                || sheet.text("Name")? != *self.sheet_name.as_ref().ok_or_else(invalid)?
            {
                return Err(invalid());
            }
            Ok(())
        }
        /// Caller must first validate the bounded, macro/link-free XLSX archive
        /// and hold a read-only source pin. The worker never selects another file.
        pub fn open_task_copy(&mut self, path: &Path) -> io::Result<()> {
            if self.book.is_some()
                || !path.is_absolute()
                || path.extension().and_then(|e| e.to_str()) != Some("xlsx")
            {
                return Err(invalid());
            }
            let path = path
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .replace('/', "\\");
            let books = self.app.as_ref().ok_or_else(invalid)?.object("Workbooks")?;
            let book = Dispatch::from(books.invoke(
                "Open",
                DISPATCH_METHOD,
                vec![path.as_str().into(), 0i32.into(), true.into()],
            )?)?;
            if book.object("Worksheets")?.number("Count")? != 1 || book.boolean("HasVBProject")? {
                return Err(invalid());
            }
            let sheet = book.object("Worksheets")?.item(1)?;
            let sheet_name = sheet.text("Name")?;
            self.source = Some(path.clone().into());
            self.current = Some(path.into());
            self.sheet_name = Some(sheet_name);
            self.sheet = Some(sheet);
            self.book = Some(book);
            self.current()
        }
        pub fn new_task_workbook(&mut self) -> io::Result<()> {
            if self.book.is_some() {
                return Err(invalid());
            }
            let books = self.app.as_ref().ok_or_else(invalid)?.object("Workbooks")?;
            let book =
                Dispatch::from(books.invoke("Add", DISPATCH_METHOD, vec![(-4167i32).into()])?)?;
            let sheet = book.object("Worksheets")?.item(1)?;
            self.current = Some(book.text("FullName")?.into());
            self.sheet_name = Some(sheet.text("Name")?);
            self.sheet = Some(sheet);
            self.book = Some(book);
            Ok(())
        }
        fn cell(&self, row: u16, col: u16) -> io::Result<Dispatch> {
            let address = address(row, col).ok_or_else(invalid)?;
            Dispatch::from(self.sheet.as_ref().ok_or_else(invalid)?.invoke(
                "Range",
                DISPATCH_PROPERTYGET | DISPATCH_METHOD,
                vec![address.as_str().into()],
            )?)
        }
        fn read_cells(&self) -> io::Result<Snapshot> {
            let sheet = self.sheet.as_ref().ok_or_else(invalid)?;
            let used = sheet.object("UsedRange")?;
            let last_row = used.number("Row")? + used.object("Rows")?.number("Count")? - 1;
            let last_col = used.number("Column")? + used.object("Columns")?.number("Count")? - 1;
            if !(1..=16).contains(&last_row) || !(1..=16).contains(&last_col) {
                return Err(invalid());
            }
            let mut cells = Vec::new();
            let rectangle = format!(
                "A1:{}",
                address(last_row as u16, last_col as u16).ok_or_else(invalid)?
            );
            let range = Dispatch::from(sheet.invoke(
                "Range",
                DISPATCH_PROPERTYGET,
                vec![rectangle.as_str().into()],
            )?)?;
            let values = values(range.get("Value2")?, last_row, last_col)?;
            for row in 1..=last_row as u16 {
                let row_address = format!(
                    "A{row}:{}",
                    address(row, last_col as u16).ok_or_else(invalid)?
                );
                let row_range = Dispatch::from(sheet.invoke(
                    "Range",
                    DISPATCH_PROPERTYGET,
                    vec![row_address.as_str().into()],
                )?)?;
                let row_format = row_range.get("NumberFormat")?;
                let row_format = if row_format.vt() == VT_BSTR {
                    Some(
                        BSTR::try_from(&row_format)
                            .map_err(|_| invalid())?
                            .to_string(),
                    )
                } else {
                    None
                };
                for col in 1..=last_col as u16 {
                    let raw = &values[(row as usize - 1) * last_col as usize + col as usize - 1];
                    if matches!(raw.vt(), VT_EMPTY | VT_NULL)
                        && row_format.as_deref() == Some("General")
                    {
                        continue;
                    }
                    let cell = self.cell(row, col)?;
                    let format = match &row_format {
                        Some(format) => format.clone(),
                        None => cell.text("NumberFormat")?,
                    };
                    if !matches!(format.as_str(), "General" | "@") {
                        return Err(invalid());
                    }
                    let empty = matches!(raw.vt(), VT_EMPTY | VT_NULL);
                    let (value, calculated) = if !empty && cell.boolean("HasFormula")? {
                        let formula = cell.text("Formula")?;
                        let sum = parse_sum(&formula).ok_or_else(invalid)?;
                        let number = f64::try_from(raw).map_err(|_| invalid())?;
                        if !number.is_finite() {
                            return Err(invalid());
                        }
                        (sum, Some(number))
                    } else {
                        match raw.vt() {
                            VT_EMPTY | VT_NULL => (CellValue::Empty, None),
                            VT_BSTR => {
                                let value = BSTR::try_from(raw).map_err(|_| invalid())?.to_string();
                                if value.len() > 4096
                                    || value
                                        .chars()
                                        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                                {
                                    return Err(invalid());
                                }
                                (CellValue::Text { value }, None)
                            }
                            VT_R8 | VT_R4 | VT_I2 | VT_I4 => {
                                let value = f64::try_from(raw).map_err(|_| invalid())?;
                                if !value.is_finite() || value.abs() > 1e12 {
                                    return Err(invalid());
                                }
                                (CellValue::Number { value }, None)
                            }
                            _ => return Err(invalid()),
                        }
                    };
                    if value != CellValue::Empty || format == "@" {
                        cells.push(Cell {
                            row,
                            col,
                            value,
                            calculated,
                            text_format: format == "@",
                        })
                    }
                }
            }
            Ok(Snapshot {
                sheet: self.sheet_name.clone().ok_or_else(invalid)?,
                cells,
            })
        }
        pub fn snapshot(&self) -> io::Result<Snapshot> {
            self.current()?;
            self.read_cells()
        }
        pub fn metadata(&self) -> io::Result<(i64, bool)> {
            self.current()?;
            Ok((
                self.window()?,
                self.book.as_ref().ok_or_else(invalid)?.boolean("Saved")?,
            ))
        }
        pub fn edit_cell(
            &mut self,
            expected: &Snapshot,
            row: u16,
            col: u16,
            value: &CellValue,
        ) -> io::Result<Snapshot> {
            self.current()?;
            // This adapter operates only on its new, hidden instance. Never
            // disable application input: a killed COM worker must not strand
            // Excel in an unusable Interactive=false state.
            (|| {
                self.current()?;
                if &self.read_cells()? != expected {
                    return Err(invalid());
                }
                let cell = self.cell(row, col)?;
                match value {
                    CellValue::Text { value } => {
                        if value.len() > 4096
                            || value
                                .chars()
                                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                        {
                            return Err(invalid());
                        }
                        cell.set("NumberFormat", "@".into())?;
                        cell.set("Value2", value.as_str().into())?;
                    }
                    CellValue::Number { value } => {
                        if !value.is_finite() || value.abs() > 1e12 {
                            return Err(invalid());
                        }
                        cell.set("NumberFormat", "General".into())?;
                        cell.set("Value2", (*value).into())?;
                    }
                    CellValue::Sum {
                        first_row,
                        first_col,
                        last_row,
                        last_col,
                    } => {
                        let formula = sum_formula(*first_row, *first_col, *last_row, *last_col)
                            .ok_or_else(invalid)?;
                        cell.set("NumberFormat", "General".into())?;
                        cell.set("Formula", formula.as_str().into())?;
                    }
                    CellValue::Empty => {
                        cell.set("Value2", VARIANT::default())?;
                    }
                }
                self.app
                    .as_ref()
                    .ok_or_else(invalid)?
                    .method("Calculate", vec![])?;
                self.read_cells()
            })()
        }
        pub fn save_new(&mut self, path: &Path, expected: &Snapshot) -> io::Result<()> {
            self.current()?;
            if !path.is_absolute()
                || path.extension().and_then(|e| e.to_str()) != Some("xlsx")
                || path.try_exists()?
                || self.snapshot()? != *expected
            {
                return Err(invalid());
            }
            let path = path
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .replace('/', "\\");
            self.book
                .as_ref()
                .ok_or_else(invalid)?
                .method("SaveAs", vec![path.as_str().into(), 51i32.into()])?;
            self.current = Some(path.into());
            if !self.book.as_ref().ok_or_else(invalid)?.boolean("Saved")?
                || self.snapshot()? != *expected
            {
                return Err(invalid());
            }
            Ok(())
        }
        pub fn close(&mut self, expected: &Snapshot) -> io::Result<()> {
            self.current()?;
            if !self.book.as_ref().ok_or_else(invalid)?.boolean("Saved")?
                || self.snapshot()? != *expected
            {
                return Err(invalid());
            }
            self.sheet = None;
            self.book
                .as_ref()
                .ok_or_else(invalid)?
                .method("Close", vec![false.into()])?;
            self.book = None;
            let app = self.app.as_ref().ok_or_else(invalid)?;
            if app.object("Workbooks")?.number("Count")? != 0 {
                return Err(invalid());
            }
            app.method("Quit", vec![])?;
            self.app = None;
            Ok(())
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn bulk_values_require_exact_bounded_shape_and_preserve_coordinates() {
            use windows::Win32::System::{
                Com::SAFEARRAYBOUND,
                Ole::{SafeArrayCreate, SafeArrayPutElement},
            };
            let bounds = [
                SAFEARRAYBOUND {
                    cElements: 2,
                    lLbound: 1,
                },
                SAFEARRAYBOUND {
                    cElements: 3,
                    lLbound: 1,
                },
            ];
            // SAFETY: OleAut32 owns a two-dimensional VARIANT array. Attach it
            // immediately to one owned VARIANT, including on assertion failure.
            let array = unsafe { SafeArrayCreate(VT_VARIANT, 2, bounds.as_ptr()) };
            assert!(!array.is_null());
            // SafeArrayCreate takes logical dimensions in declaration order.
            // Explicitly establish the rectangular fixture shape before filling it.
            // SAFETY: nonnull live array returned by SafeArrayCreate above; valid dimension.
            assert_eq!(unsafe { SafeArrayGetUBound(array, 1) }.unwrap(), 2);
            // SAFETY: same live two-dimensional array; read-only bound query.
            assert_eq!(unsafe { SafeArrayGetUBound(array, 2) }.unwrap(), 3);
            let mut raw = VARIANT::default();
            // SAFETY: attach the unique array to an empty owner, then copy bounded
            // VARIANT elements through OleAut32. The array owner outlives all writes.
            unsafe {
                raw.Anonymous.Anonymous =
                    std::mem::ManuallyDrop::new(windows::Win32::System::Variant::VARIANT_0_0 {
                        vt: windows::Win32::System::Variant::VARENUM(VT_ARRAY.0 | VT_VARIANT.0),
                        Anonymous: windows::Win32::System::Variant::VARIANT_0_0_0 { parray: array },
                        ..Default::default()
                    });
                for row in 1..=2 {
                    for col in 1..=3 {
                        let value = VARIANT::from(row * 10 + col);
                        SafeArrayPutElement(
                            array,
                            [row, col].as_ptr(),
                            (&value as *const VARIANT).cast(),
                        )
                        .unwrap();
                    }
                }
            }
            assert!(values(raw.clone(), 3, 2).is_err());
            let result = values(raw, 2, 3).unwrap();
            let actual = result
                .iter()
                .map(|v| i32::try_from(v).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(actual, [11, 12, 13, 21, 22, 23]);
            assert!(values(VARIANT::from(1i32), 17, 1).is_err());
            assert!(values(VARIANT::from(1i32), 2, 1).is_err());
            assert_eq!(
                i32::try_from(&values(VARIANT::from(7i32), 1, 1).unwrap()[0]).unwrap(),
                7
            );
        }
    }
}
