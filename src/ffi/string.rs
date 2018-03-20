use core::{fmt, mem, ops, ptr, slice};
use core::borrow::Borrow;
use core::slice::memchr;
use core::str::Utf8Error;
use ffi::{c_char, strlen, CStr};

/// A type representing an owned, C-compatible, nul-terminated string with no
/// nul bytes in the middle.
///
/// This type serves the purpose of being able to safely generate a C-compatible
/// string from a Rust byte slice or vector. An instance of this type is a
/// static guarantee that the underlying bytes contain no interior 0 bytes ("nul
/// characters") and that the final byte is 0 ("nul terminator").
///
/// `CString` is to [`CStr`] as `String` is to `&str`: the former in each pair
/// are owned strings; the latter are borrowed references.
///
/// # Creating a `CString`
///
/// A `CString` is created from either a byte slice or a byte vector, or
/// anything that implements `Into<Vec<u8>>` (for example, you can build a
/// `CString` straight out of a `String` or a `&str`, since both implement that
/// trait).
///
/// The [`new`] method will actually check that the provided `&u8` does not have
/// 0 bytes in the middle, and return an error if it finds one.
///
/// # Extracting a raw pointer to the whole C string
///
/// `CString` implements a [`as_ptr`] method through the `Deref` trait. This
/// method will give you a `*const c_char` which you can feed directly to extern
/// functions that expect a nul-terminated string, like C's `strdup()`.
///
/// # Extracting a slice of the whole C string
///
/// Alternatively, you can obtain a `&[u8]` slice from a `CString` with the
/// [`as_bytes`] method. Slices produced in this way do *not* contain the
/// trailing nul terminator. This is useful when you will be calling an extern
/// function that takes a `*const u8` argument which is not necessarily
/// nul-terminated, plus another argument with the length of the string — like
/// C's `strndup()`.  You can of course get the slice's length with its `len`
/// method.
///
/// If you need a `&[u8]` slice *with* the nul terminator, you can use
/// [`as_bytes_with_nul`] instead.
///
/// Once you have the kind of slice you need (with or without a nul terminator),
/// you can call the slice's own `as_ptr` method to get a raw pointer to pass to
/// extern functions. See the documentation for that function for a discussion
/// on ensuring the lifetime of the raw pointer.
///
/// [`new`]: CString::new
/// [`as_bytes`]: CString::as_bytes
/// [`as_bytes_with_nul`]: CString::as_bytes_with_nul
/// [`as_ptr`]: CString::as_ptr
/// [`CStr`]: CStr
///
/// # Examples
///
/// ```
/// use drone_core::ffi::{CString, c_char};
///
/// unsafe fn my_printer(s: *const c_char) {}
///
/// // We are certain that our string doesn't have 0 bytes in the middle, so we
/// // can .unwrap()
/// let c_to_print = CString::new("Hello, world!").unwrap();
/// unsafe {
///   my_printer(c_to_print.as_ptr());
/// }
/// ```
///
/// # Safety
///
/// `CString` is intended for working with traditional C-style strings (a
/// sequence of non-nul bytes terminated by a single nul byte); the primary use
/// case for these kinds of strings is interoperating with C-like code. Often
/// you will need to transfer ownership to/from that external code. It is
/// strongly recommended that you thoroughly read through the documentation of
/// `CString` before use, as improper ownership management of `CString`
/// instances can lead to invalid memory accesses, memory leaks, and other
/// memory errors.
#[derive(PartialEq, PartialOrd, Eq, Ord, Hash, Clone)]
pub struct CString {
  pub(super) inner: Box<[u8]>,
}

/// An error indicating that an interior nul byte was found.
///
/// While Rust strings may contain nul bytes in the middle, C strings can't, as
/// that byte would effectively truncate the string.
///
/// This error is created by the [`new`][`CString::new`] method on [`CString`].
/// See its documentation for more.
///
/// [`CString`]: CString
/// [`CString::new`]: CString::new
///
/// # Examples
///
/// ```
/// use drone_core::ffi::{CString, NulError};
///
/// let _: NulError = CString::new(b"f\0oo".to_vec()).unwrap_err();
/// ```
#[derive(Clone, PartialEq, Eq, Debug, Fail)]
#[fail(display = "nul byte found in provided data at position: {}", _0)]
pub struct NulError(usize, Vec<u8>);

/// An error indicating invalid UTF-8 when converting a [`CString`] into a
/// `String`.
///
/// `CString` is just a wrapper over a buffer of bytes with a nul terminator;
/// [`into_string`][`CString::into_string`] performs UTF-8 validation on those
/// bytes and may return this error.
///
/// This `struct` is created by the [`into_string`][`CString::into_string`]
/// method on [`CString`]. See its documentation for more.
///
/// [`CString`]: CString
/// [`CString::into_string`]: CString::into_string
#[derive(Clone, PartialEq, Eq, Debug, Fail)]
#[fail(display = "C string contained non-utf8 bytes")]
pub struct IntoStringError {
  inner: CString,
  error: Utf8Error,
}

impl CString {
  /// Creates a new C-compatible string from a container of bytes.
  ///
  /// This function will consume the provided data and use the underlying bytes
  /// to construct a new string, ensuring that there is a trailing 0 byte. This
  /// trailing 0 byte will be appended by this function; the provided data
  /// should *not* contain any 0 bytes in it.
  ///
  /// # Examples
  ///
  /// ```
  /// use drone_core::ffi::{CString, c_char};
  ///
  /// unsafe fn puts(_s: *const c_char) {}
  ///
  /// let to_print = CString::new("Hello!").unwrap();
  /// unsafe {
  ///   puts(to_print.as_ptr());
  /// }
  /// ```
  ///
  /// # Errors
  ///
  /// This function will return an error if the supplied bytes contain an
  /// internal 0 byte. The [`NulError`] returned will contain the bytes as well as
  /// the position of the nul byte.
  ///
  /// [`NulError`]: NulError
  pub fn new<T: Into<Vec<u8>>>(t: T) -> Result<CString, NulError> {
    Self::_new(t.into())
  }

  fn _new(bytes: Vec<u8>) -> Result<CString, NulError> {
    match memchr::memchr(0, &bytes) {
      Some(i) => Err(NulError(i, bytes)),
      None => Ok(unsafe { CString::from_vec_unchecked(bytes) }),
    }
  }

  /// Creates a C-compatible string by consuming a byte vector, without checking
  /// for interior 0 bytes.
  ///
  /// This method is equivalent to [`new`] except that no runtime assertion is
  /// made that `v` contains no 0 bytes, and it requires an actual byte vector,
  /// not anything that can be converted to one with Into.
  ///
  /// [`new`]: CString::new
  ///
  /// # Examples
  ///
  /// ```
  /// use drone_core::ffi::CString;
  ///
  /// let raw = b"foo".to_vec();
  /// unsafe {
  ///   let c_string = CString::from_vec_unchecked(raw);
  /// }
  /// ```
  pub unsafe fn from_vec_unchecked(mut v: Vec<u8>) -> CString {
    v.reserve_exact(1);
    v.push(0);
    CString {
      inner: v.into_boxed_slice(),
    }
  }

  /// Retakes ownership of a `CString` that was transferred to C via
  /// [`into_raw`].
  ///
  /// Additionally, the length of the string will be recalculated from the
  /// pointer.
  ///
  /// # Safety
  ///
  /// This should only ever be called with a pointer that was earlier obtained
  /// by calling [`into_raw`] on a `CString`. Other usage (e.g. trying to take
  /// ownership of a string that was allocated by foreign code) is likely to
  /// lead to undefined behavior or allocator corruption.
  ///
  /// > **Note:** If you need to borrow a string that was allocated by foreign
  /// > code, use [`CStr`]. If you need to take ownership of a string that was
  /// > allocated by foreign code, you will need to make your own provisions for
  /// > freeing it appropriately, likely with the foreign code's API to do that.
  ///
  /// [`into_raw`]: CString::into_raw
  /// [`CStr`]: CStr
  ///
  /// # Examples
  ///
  /// Create a `CString`, pass ownership to an `extern` function (via raw
  /// pointer), then retake ownership with `from_raw`:
  ///
  /// ```
  /// use drone_core::ffi::{CString, c_char};
  ///
  /// unsafe fn some_extern_function(_s: *mut c_char) {}
  ///
  /// let c_string = CString::new("Hello!").unwrap();
  /// let raw = c_string.into_raw();
  /// unsafe {
  ///   some_extern_function(raw);
  ///   let c_string = CString::from_raw(raw);
  /// }
  /// ```
  pub unsafe fn from_raw(ptr: *mut c_char) -> CString {
    let len = strlen(ptr) + 1; // Including the NUL byte
    let slice = slice::from_raw_parts_mut(ptr, len as usize);
    CString {
      inner: Box::from_raw(slice as *mut [c_char] as *mut [u8]),
    }
  }

  /// Consumes the `CString` and transfers ownership of the string to a C
  /// caller.
  ///
  /// The pointer which this function returns must be returned to Rust and
  /// reconstituted using [`from_raw`] to be properly deallocated. Specifically,
  /// one should *not* use the standard C `free()` function to deallocate this
  /// string.
  ///
  /// Failure to call [`from_raw`] will lead to a memory leak.
  ///
  /// [`from_raw`]: CString::from_raw
  ///
  /// # Examples
  ///
  /// ```
  /// use drone_core::ffi::CString;
  ///
  /// let c_string = CString::new("foo").unwrap();
  ///
  /// let ptr = c_string.into_raw();
  ///
  /// unsafe {
  ///   assert_eq!(b'f', *ptr as u8);
  ///   assert_eq!(b'o', *ptr.offset(1) as u8);
  ///   assert_eq!(b'o', *ptr.offset(2) as u8);
  ///   assert_eq!(b'\0', *ptr.offset(3) as u8);
  ///
  ///   // retake pointer to free memory
  ///   let _ = CString::from_raw(ptr);
  /// }
  /// ```
  #[inline]
  pub fn into_raw(self) -> *mut c_char {
    Box::into_raw(self.into_inner()) as *mut c_char
  }

  /// Converts the `CString` into a `String` if it contains valid UTF-8 data.
  ///
  /// On failure, ownership of the original `CString` is returned.
  ///
  /// # Examples
  ///
  /// ```
  /// use drone_core::ffi::CString;
  ///
  /// let valid_utf8 = vec![b'f', b'o', b'o'];
  /// let cstring = CString::new(valid_utf8).unwrap();
  /// assert_eq!(cstring.into_string().unwrap(), "foo");
  ///
  /// let invalid_utf8 = vec![b'f', 0xff, b'o', b'o'];
  /// let cstring = CString::new(invalid_utf8).unwrap();
  /// let err = cstring.into_string().err().unwrap();
  /// assert_eq!(err.utf8_error().valid_up_to(), 1);
  /// ```
  pub fn into_string(self) -> Result<String, IntoStringError> {
    String::from_utf8(self.into_bytes()).map_err(|e| IntoStringError {
      error: e.utf8_error(),
      inner: unsafe { CString::from_vec_unchecked(e.into_bytes()) },
    })
  }

  /// Consumes the `CString` and returns the underlying byte buffer.
  ///
  /// The returned buffer does **not** contain the trailing nul terminator, and
  /// it is guaranteed to not have any interior nul bytes.
  ///
  /// # Examples
  ///
  /// ```
  /// use drone_core::ffi::CString;
  ///
  /// let c_string = CString::new("foo").unwrap();
  /// let bytes = c_string.into_bytes();
  /// assert_eq!(bytes, vec![b'f', b'o', b'o']);
  /// ```
  pub fn into_bytes(self) -> Vec<u8> {
    let mut vec = self.into_inner().into_vec();
    let _nul = vec.pop();
    debug_assert_eq!(_nul, Some(0u8));
    vec
  }

  /// Equivalent to the [`into_bytes`] function except that the returned vector
  /// includes the trailing nul terminator.
  ///
  /// [`into_bytes`]: CString::into_bytes
  ///
  /// # Examples
  ///
  /// ```
  /// use drone_core::ffi::CString;
  ///
  /// let c_string = CString::new("foo").unwrap();
  /// let bytes = c_string.into_bytes_with_nul();
  /// assert_eq!(bytes, vec![b'f', b'o', b'o', b'\0']);
  /// ```
  pub fn into_bytes_with_nul(self) -> Vec<u8> {
    self.into_inner().into_vec()
  }

  /// Returns the contents of this `CString` as a slice of bytes.
  ///
  /// The returned slice does **not** contain the trailing nul terminator, and
  /// it is guaranteed to not have any interior nul bytes. If you need the nul
  /// terminator, use [`as_bytes_with_nul`] instead.
  ///
  /// [`as_bytes_with_nul`]: CString::as_bytes_with_nul
  ///
  /// # Examples
  ///
  /// ```
  /// use drone_core::ffi::CString;
  ///
  /// let c_string = CString::new("foo").unwrap();
  /// let bytes = c_string.as_bytes();
  /// assert_eq!(bytes, &[b'f', b'o', b'o']);
  /// ```
  #[inline]
  pub fn as_bytes(&self) -> &[u8] {
    &self.inner[..self.inner.len() - 1]
  }

  /// Equivalent to the [`as_bytes`] function except that the returned slice
  /// includes the trailing nul terminator.
  ///
  /// [`as_bytes`]: CString::as_bytes
  ///
  /// # Examples
  ///
  /// ```
  /// use drone_core::ffi::CString;
  ///
  /// let c_string = CString::new("foo").unwrap();
  /// let bytes = c_string.as_bytes_with_nul();
  /// assert_eq!(bytes, &[b'f', b'o', b'o', b'\0']);
  /// ```
  #[inline]
  pub fn as_bytes_with_nul(&self) -> &[u8] {
    &self.inner
  }

  /// Extracts a [`CStr`] slice containing the entire string.
  ///
  /// [`CStr`]: CStr
  ///
  /// # Examples
  ///
  /// ```
  /// use drone_core::ffi::{CString, CStr};
  ///
  /// let c_string = CString::new(b"foo".to_vec()).unwrap();
  /// let c_str = c_string.as_c_str();
  /// assert_eq!(c_str, CStr::from_bytes_with_nul(b"foo\0").unwrap());
  /// ```
  #[inline]
  pub fn as_c_str(&self) -> &CStr {
    &*self
  }

  /// Converts this `CString` into a boxed [`CStr`].
  ///
  /// [`CStr`]: CStr
  ///
  /// # Examples
  ///
  /// ```
  /// use drone_core::ffi::{CString, CStr};
  ///
  /// let c_string = CString::new(b"foo".to_vec()).unwrap();
  /// let boxed = c_string.into_boxed_c_str();
  /// assert_eq!(&*boxed, CStr::from_bytes_with_nul(b"foo\0").unwrap());
  /// ```
  pub fn into_boxed_c_str(self) -> Box<CStr> {
    unsafe { Box::from_raw(Box::into_raw(self.into_inner()) as *mut CStr) }
  }

  // Bypass "move out of struct which implements `Drop` trait" restriction.
  pub(super) fn into_inner(self) -> Box<[u8]> {
    unsafe {
      let result = ptr::read(&self.inner);
      mem::forget(self);
      result
    }
  }
}

impl NulError {
  /// Returns the position of the nul byte in the slice that caused
  /// [`CString::new`] to fail.
  ///
  /// [`CString::new`]: CString::new
  ///
  /// # Examples
  ///
  /// ```
  /// use drone_core::ffi::CString;
  ///
  /// let nul_error = CString::new("foo\0bar").unwrap_err();
  /// assert_eq!(nul_error.nul_position(), 3);
  ///
  /// let nul_error = CString::new("foo bar\0").unwrap_err();
  /// assert_eq!(nul_error.nul_position(), 7);
  /// ```
  pub fn nul_position(&self) -> usize {
    self.0
  }

  /// Consumes this error, returning the underlying vector of bytes which
  /// generated the error in the first place.
  ///
  /// # Examples
  ///
  /// ```
  /// use drone_core::ffi::CString;
  ///
  /// let nul_error = CString::new("foo\0bar").unwrap_err();
  /// assert_eq!(nul_error.into_vec(), b"foo\0bar");
  /// ```
  pub fn into_vec(self) -> Vec<u8> {
    self.1
  }
}

impl IntoStringError {
  /// Consumes this error, returning original [`CString`] which generated the
  /// error.
  ///
  /// [`CString`]: CString
  pub fn into_cstring(self) -> CString {
    self.inner
  }

  /// Access the underlying UTF-8 error that was the cause of this error.
  pub fn utf8_error(&self) -> Utf8Error {
    self.error
  }
}

// Turns this `CString` into an empty string to prevent
// memory unsafe code from working by accident. Inline
// to prevent LLVM from optimizing it away in debug builds.
impl Drop for CString {
  #[inline]
  fn drop(&mut self) {
    unsafe { *self.inner.get_unchecked_mut(0) = 0 };
  }
}

impl ops::Deref for CString {
  type Target = CStr;

  #[inline]
  fn deref(&self) -> &CStr {
    unsafe { CStr::from_bytes_with_nul_unchecked(self.as_bytes_with_nul()) }
  }
}

impl fmt::Debug for CString {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    fmt::Debug::fmt(&**self, f)
  }
}

impl From<CString> for Vec<u8> {
  #[inline]
  fn from(s: CString) -> Vec<u8> {
    s.into_bytes()
  }
}

impl Default for CString {
  /// Creates an empty `CString`.
  fn default() -> CString {
    let a: &CStr = Default::default();
    a.to_owned()
  }
}

impl Borrow<CStr> for CString {
  #[inline]
  fn borrow(&self) -> &CStr {
    self
  }
}

impl From<Box<CStr>> for CString {
  #[inline]
  fn from(s: Box<CStr>) -> CString {
    s.into_c_string()
  }
}

impl<'a> From<&'a CStr> for CString {
  fn from(s: &'a CStr) -> CString {
    s.to_owned()
  }
}

impl ops::Index<ops::RangeFull> for CString {
  type Output = CStr;

  #[inline]
  fn index(&self, _index: ops::RangeFull) -> &CStr {
    self
  }
}

impl AsRef<CStr> for CString {
  #[inline]
  fn as_ref(&self) -> &CStr {
    self
  }
}