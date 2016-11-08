use std;
use std::{ptr,marker,slice,cell};
use libc;
use ::{FlowError,FlowErr,JsonResponse,JsonResponseError,Result,IoDirection};
extern crate imageflow_serde as s;
extern crate serde_json;

pub struct ContextPtr {
    // TODO: Remove pub as soon as tests/visuals.rs doesn't need access
    // (i.e, unit test helpers are ported, or the helper becomes cfgtest on the struct itself)
    pub ptr: Option<*mut ::ffi::Context>,
}
pub struct Context {
    p: cell::RefCell<ContextPtr>,
}

pub struct JobPtr {
    ptr: *mut ::ffi::Job,
    c: *mut ::ffi::Context
}

impl JobPtr {
    pub fn context_ptr(&self) -> *mut ::ffi::Context{ self.c }
    pub fn as_ptr(&self) -> *mut ::ffi::Job { self.ptr}

    pub fn from_ptr(context: *mut ::ffi::Context, job: *mut ::ffi::Job) -> Result<JobPtr> {
        if context.is_null() || job.is_null() {
            Err(FlowError::NullArgument)
        }else {
            Ok(JobPtr {
                ptr: job,
                c: context
            })
        }
    }
    pub fn create(context: *mut ::ffi::Context) -> Result<JobPtr> {
        if context.is_null() {
            return Err(FlowError::ContextInvalid)
        }
        unsafe {
            let job = ::ffi::flow_job_create(context);
            if job.is_null() {
                Err(FlowError::Oom)
            }else{
                Ok(JobPtr { ptr: job, c: context})
            }
        }
    }
    pub fn configure_graph_recording(&self, r: s::Build001GraphRecording){
        let _ = unsafe { ::ffi::flow_job_configure_recording(self.context_ptr(),
                                                    self.as_ptr(),
                                                    r.record_graph_versions
                                                        .unwrap_or(false),
                                                    r.record_frame_images
                                                        .unwrap_or(false),
                                                    r.render_last_graph
                                                        .unwrap_or(false),
                                                    r.render_graph_versions
                                                        .unwrap_or(false),
                                                    r.render_animated_graph
                                                        .unwrap_or(false)) };
    }

    pub fn get_image_info(&self, io_id: i32) -> Result<s::ImageInfo> {
        unsafe {
            let mut info: ::ffi::DecoderInfo = ::ffi::DecoderInfo { ..Default::default() };

            if !::ffi::flow_job_get_decoder_info(self.context_ptr(), self.as_ptr(), 0, &mut info) {
                ContextPtr::from_ptr(self.context_ptr()).assert_ok(None);
            }
            Ok(s::ImageInfo {
                frame0_post_decode_format: s::PixelFormat::from(info.frame0_post_decode_format),
                frame0_height: info.frame0_height,
                frame0_width: info.frame0_width,
                frame_count: info.frame_count,
                current_frame_index: info.current_frame_index,
                preferred_extension: std::ffi::CStr::from_ptr(info.preferred_extension).to_owned().into_string().unwrap(),
                preferred_mime_type: std::ffi::CStr::from_ptr(info.preferred_mime_type).to_owned().into_string().unwrap(),
            })
        }

    }

    pub fn tell_decoder(&self, io_id: i32, tell: s::TellDecoderWhat ) -> Result<()> {
        unsafe {
            match tell {
                s::TellDecoderWhat::JpegDownscaleHints(hints) => {
                    if !::ffi::flow_job_decoder_set_downscale_hints_by_placeholder_id(self.context_ptr(),
                                                                                      self.as_ptr(), io_id,
                                                                                      hints.width, hints.height,
                                                                                      hints.width, hints.height,
                                                                                      hints.scale_luma_spatially.unwrap_or(false),
                                                                                      hints.gamma_correct_for_srgb_during_spatial_luma_scaling.unwrap_or(false)

                    ){
                        panic!("");
                    }
                }
            }
        }
        Ok(())

    }


    pub fn message<'a, 'b, 'c>(&'a mut self,
                               method: &'b str,
                               json: &'b [u8])
                               -> Result<JsonResponse<'c>> {

        match method {
            "v0.0.1/get_image_info" => {
                let parsed: s::GetImageInfo001 = serde_json::from_slice(json).unwrap();
                let info = self.get_image_info(parsed.io_id).unwrap();
                Ok(JsonResponse::success_with_payload(s::ResponsePayload::ImageInfo(info)))
            }
            "v0.0.1/tell_decoder" => {
                let parsed: s::TellDecoder001 = serde_json::from_slice(json).unwrap();
                self.tell_decoder(parsed.io_id, parsed.command).unwrap();
                Ok(JsonResponse::ok())
            }
            "brew_coffee" => Ok(JsonResponse::teapot()),
            _ => Ok(JsonResponse::method_not_understood())
        }
    }
}

pub struct Job {
    pub p: cell::RefCell<JobPtr>,
}
pub struct JobIoPtr {
    pub ptr: Option<*mut ::ffi::JobIO>,
}

pub struct JobIo<'a, T: 'a> {
    pub p: cell::RefCell<JobIoPtr>,
    pub _marker: marker::PhantomData<&'a T>,
}



impl Context {
    pub fn message<'a, 'b, 'c>(&'a mut self,
                               method: &'b str,
                               json: &'b [u8])
                               -> Result<JsonResponse> {
        let ref mut b = *self.p.borrow_mut();
        b.message(method, json)
    }
}

impl ContextPtr {
    pub fn message<'a, 'b, 'c>(&'a mut self,
                               method: &'b str,
                               json: &'b [u8])
                               -> Result<JsonResponse<'c>> {
        if self.ptr.is_none() {
            return Err(FlowError::ContextInvalid);
        }
        let response = match method {
            "brew_coffee" => JsonResponse::teapot(),
            "v0.0.1/build" => unsafe {

                let handler = ::parsing::BuildRequestHandler::new();
                let response = handler.do_and_respond(&mut *self, json);
                self.assert_ok(None);

                response.unwrap()
            },
            _ => JsonResponse::method_not_understood()
        };
        Ok(response)
    }

    fn build_0_0_1<'a, 'b, 'c>(&'a mut self, json: &'b [u8]) -> Result<JsonResponse<'c>> {
        match ::parsing::BuildRequestHandler::new().do_and_respond(self, json) {
            Ok(response) => Ok(response),
            Err(original_err) => {
                Err(match original_err {
                    JsonResponseError::Oom(()) => FlowError::Oom,
                    JsonResponseError::NotImplemented(()) => FlowError::ErrNotImpl,
                    JsonResponseError::Other(e) => FlowError::ErrNotImpl,
                })
            }
        }
    }
}

impl ContextPtr {
    fn destroy(&mut self) {
        unsafe {
            self.ptr = match self.ptr {
                Some(ptr) => {
                    ::ffi::flow_context_destroy(ptr);
                    None
                }
                _ => None,
            }
        }
    }

    pub fn from_ptr(ptr: *mut ::ffi::Context) -> ContextPtr {
        ContextPtr {
            ptr: match ptr.is_null() {
                false => Some(ptr),
                true => None,
            },
        }
    }
}



impl Drop for Context {
    fn drop(&mut self) {
        (*self.p.borrow_mut()).destroy();
    }
}
impl Context {
    pub fn create() -> Result<Context> {
        unsafe {
            let ptr = ::ffi::flow_context_create();

            if ptr.is_null() {
                Err(FlowError::Oom)
            } else {
                Ok(Context { p: cell::RefCell::new(ContextPtr { ptr: Some(ptr) }) })
            }
        }
    }

    pub fn unsafe_borrow_mut_context_pointer(&mut self) -> std::cell::RefMut<ContextPtr> {
        self.p.borrow_mut()
    }

    fn get_error_copy(&self) -> Option<FlowError> {
        (*self.p.borrow()).get_error_copy()
    }

    pub fn destroy(self) -> Result<()> {
        let ref mut b = *self.p.borrow_mut();
        match b.ptr {
            None => Ok(()),
            Some(ptr) => unsafe {
                if !::ffi::flow_context_begin_terminate(ptr) {
                    // Already borrowed; will panic!
                    // This kind of bug is only exposed at runtime, now.
                    // Code reuse will require two copies of every function
                    // One against the ContextPtr, to be reused
                    // One exposed publicly against the Context, which performs the borrowing
                    // Same scenario will occur with other types.
                    // let copy = self.get_error_copy().unwrap();

                    // So use the ContextPtr version
                    let copy = b.get_error_copy().unwrap();
                    b.destroy();
                    Err(copy)
                } else {
                    b.destroy();
                    Ok(())
                }
            },
        }
    }

    pub fn create_job(&mut self) -> Result<Job> {
        let ref b = *self.p.borrow_mut();
        match b.ptr {
            None => Err(FlowError::ContextInvalid),
            Some(ptr) => unsafe {
                let p = ::ffi::flow_job_create(ptr);
                if p.is_null() {
                    Err(b.get_error_copy().unwrap())
                } else {
                    Ok(Job { p: cell::RefCell::new(JobPtr::from_ptr(ptr, p).unwrap()) })
                }
            },
        }
    }


    pub fn create_io_from_slice<'a, 'c>(&'c mut self,
                                        bytes: &'a [u8])
                                        -> Result<JobIo<'a, &'a [u8]>> {
        let ref b = *self.p.borrow_mut();
        match b.ptr {
            None => Err(FlowError::ContextInvalid),
            Some(ptr) => unsafe {
                let p = ::ffi::flow_io_create_from_memory(ptr,
                                                          ::ffi::IoMode::read_seekable,
                                                          bytes.as_ptr(),
                                                          bytes.len(),
                                                          ptr as *const libc::c_void,
                                                          ptr::null());
                if p.is_null() {
                    Err(b.get_error_copy().unwrap())
                } else {
                    Ok(JobIo {
                        _marker: marker::PhantomData,
                        p: cell::RefCell::new(JobIoPtr { ptr: Some(p) }),
                    })
                }
            },
        }
    }


    pub fn create_io_output_buffer<'a, 'b>(&'a mut self) -> Result<JobIo<'b, ()>> {
        let ref b = *self.p.borrow_mut();
        match b.ptr {
            None => Err(FlowError::ContextInvalid),
            Some(ptr) => unsafe {
                let p = ::ffi::flow_io_create_for_output_buffer(ptr, ptr as *const libc::c_void);
                if p.is_null() {
                    Err(b.get_error_copy().unwrap())
                } else {
                    Ok(JobIo {
                        _marker: marker::PhantomData,
                        p: cell::RefCell::new(JobIoPtr { ptr: Some(p) }),
                    })
                }
            },
        }
    }

    pub fn job_add_io<T>(&mut self,
                         job: &mut Job,
                         io: JobIo<T>,
                         io_id: i32,
                         direction: IoDirection)
                         -> Result<()> {
        let ref b = *self.p.borrow_mut();
        match b.ptr {
            None => Err(FlowError::ContextInvalid),
            Some(ptr) => unsafe {
                let p = ::ffi::flow_job_add_io(ptr,
                                               (*job.p.borrow_mut()).ptr,
                                               (*io.p.borrow_mut()).ptr.unwrap(),
                                               io_id,
                                               direction);
                if !p {
                    Err(b.get_error_copy().unwrap())
                } else {
                    Ok(())
                }
            },
        }
    }

    pub fn io_get_output_buffer<'a, 'b>(&'a mut self,
                                        job: &'b Job,
                                        io_id: i32)
                                        -> Result<&'b [u8]> {
        let ref b = *self.p.borrow_mut();
        match b.ptr {
            None => Err(FlowError::ContextInvalid),
            Some(ptr) => unsafe {

                let io_p = ::ffi::flow_job_get_io(ptr, (*job.p.borrow_mut()).ptr, io_id);
                if io_p.is_null() {
                    Err(b.get_error_copy().unwrap())
                } else {
                    let mut buf_start: *const u8 = ptr::null();
                    let mut buf_len: usize = 0;
                    let worked = ::ffi::flow_io_get_output_buffer(ptr,
                                                                  io_p,
                                                                  &mut buf_start as *mut *const u8,
                                                                  &mut buf_len as *mut usize);
                    if !worked {
                        Err(b.get_error_copy().unwrap())
                    } else {
                        if buf_start.is_null() {
                            // Not sure how output buffer is null... no writes yet?
                            Err(FlowError::ErrNotImpl)
                        } else {
                            Ok((std::slice::from_raw_parts(buf_start, buf_len)))
                        }
                    }
                }



            },
        }
    }
}


impl ContextPtr {
    unsafe fn get_flow_err(&self, c: *mut ::ffi::Context) -> FlowErr {


        let code = ::ffi::flow_context_error_reason(c);
        let mut buf = vec![0u8; 2048];


        let chars_written =
        ::ffi::flow_context_error_and_stacktrace(c, buf.as_mut_ptr(), buf.len(), false);

        if chars_written < 0 {
            panic!("Error msg doesn't fit in 2kb");
        } else {
            buf.resize(chars_written as usize, 0u8);
        }

        FlowErr {
            code: code,
            message_and_stack: String::from_utf8(buf).unwrap(),
        }

    }


    pub unsafe fn assert_ok(&self, g: Option<&::flow::graph::Graph>) {
        match self.get_error_copy() {
            Some(which_error) => {
                match which_error {
                    FlowError::Err(e) => {

                        println!("Error {} {}\n", e.code, e.message_and_stack);
                        if e.code == 72 || e.code == 73 {
                            if g.is_some() {
                                //                                let _ = ::flow::graph::print_to_stdout(
                                //                                    self.ptr.unwrap(),
                                //                                    g.unwrap() as &flow::graph::Graph);
                            }
                        }

                        panic!();
                    }
                    FlowError::Oom => {
                        panic!("Out of memory.");
                    }
                    FlowError::ErrNotImpl => {
                        panic!("Error not implemented");
                    }
                    FlowError::ContextInvalid => {
                        panic!("Context pointer null");
                    }
                    FlowError::NullArgument => {
                        panic!("Context pointer null");
                    }

                }
            }
            None => {}
        }
    }


    fn get_error_copy(&self) -> Option<FlowError> {
        unsafe {
            match self.ptr {
                Some(ptr) if ::ffi::flow_context_has_error(ptr) => {
                    match ::ffi::flow_context_error_reason(ptr) {
                        0 => panic!("Inconsistent errors"),
                        10 => Some(FlowError::Oom),
                        _ => Some(FlowError::Err(self.get_flow_err(ptr))),
                    }
                }
                None => Some(FlowError::ContextInvalid),
                Some(_) => None,
            }
        }
    }
}



#[test]
fn it_works() {
    let mut c = Context::create().unwrap();

    let mut j = c.create_job().unwrap();


    let bytes = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49,
        0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06,
        0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44,
        0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D,
        0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42,
        0x60, 0x82];

    let input = c.create_io_from_slice(&bytes).unwrap();

    let output = c.create_io_output_buffer().unwrap();

    c.job_add_io(&mut j, input, 0, IoDirection::In).unwrap();
    c.job_add_io(&mut j, output, 1, IoDirection::Out).unwrap();


    // let output_bytes = c.io_get_output_buffer(&j, 1).unwrap();

    assert_eq!(c.destroy(), Ok(()));

}