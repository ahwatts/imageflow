use ::internal_prelude::works_everywhere::*;

use ::{Context,ContextPtr, JobPtr};
use ::parsing::parse_graph::GraphTranslator;
use ::parsing::parse_io::IoTranslator;
use std::error;
use ::json::*;


fn create_context_router() -> MethodRouter<'static, ContextPtr>{
    let mut r = MethodRouter::new();
//    r.add_responder("v0.1/load_image_info", Box::new(
//        move |context: &mut ContextPtr, data: s::GetImageInfo001| {
//            Ok(JsonResponse::method_not_understood())
//            //Ok(s::ResponsePayload::ImageInfo(job.get_image_info(data.io_id)?))
//        }
//    ));
    r.add_responder("v0.1/build", Box::new(
        move |context: &mut ContextPtr, parsed: s::Build001| {
            Build_1_Handler::from_context(context).build_1(parsed)
        }
    ));
    r.add("brew_coffee", Box::new(
        move |context: &mut ContextPtr, bytes: &[u8] |{
            Ok(JsonResponse::teapot())
        }
    ));
    r
}

lazy_static! {
        pub static ref  CONTEXT_ROUTER: MethodRouter<'static, ContextPtr> = create_context_router();
    }



fn get_create_doc_dir() -> std::path::PathBuf {
    let path = Path::new(file!()).parent().unwrap().join(Path::new("../../target/doc"));
    let _ = std::fs::create_dir_all(&path);
    //Error { repr: Os { code: 17, message: "File exists" } }
    //The above can happen, despite the docs.
    path
}
#[test]
fn write_context_doc(){
    let path = get_create_doc_dir().join(Path::new("context_json_api.txt"));
    File::create(&path).unwrap().write_all(document_message().as_bytes()).unwrap();
}



fn document_message() -> String {
    let mut s = String::new();
    s.reserve(8000);
    s += "# JSON API - Context\n\n";
    s += "imageflow_context responds to these message methods\n\n";
    s += "## v0.1/build \n";
    s += "Example message body:\n";
    s += &serde_json::to_string_pretty(&s::Build001::example_with_steps()).unwrap();
    s += "\n\nExample response:\n";
    s += &serde_json::to_string_pretty(&s::Response001::example_job_result_encoded(2, 200,200, "image/png", "png")).unwrap();
    s += "\n\nExample failure response:\n";
    s += &serde_json::to_string_pretty(&s::Response001::example_error()).unwrap();
    s += "\n\n";

    s
}



pub struct Build_1_Handler<'a> {
    use_context: Option<&'a ContextPtr>
}


impl<'a> Build_1_Handler<'a> {
    pub fn new() -> Build_1_Handler<'static> {
        Build_1_Handler { use_context: None}
    }

    pub fn from_context(context: &'a mut ContextPtr) -> Build_1_Handler<'a> {
        Build_1_Handler { use_context: Some(context)}
    }

    pub fn build_1(&self, task: s::Build001) -> Result<s::ResponsePayload> {
        if self.use_context.is_none() {
            let mut ctx = ::SelfDisposingContextPtr::create().unwrap();
            self.build_inner(ctx.inner(),task)
        }else{
            self.build_inner(self.use_context.unwrap(), task)
        }
    }


    fn build_inner(&self, ctx: &ContextPtr, parsed: s::Build001)  -> Result<s::ResponsePayload>  {

        let mut g = ::parsing::GraphTranslator::new().translate_framewise(parsed.framewise)?;

        unsafe {
            let p = ctx.ptr.unwrap();
            let mut job = JobPtr::create(p).unwrap();

            if let Some(s::Build001Config{ ref no_gamma_correction, ..}) = parsed.builder_config {
                ::ffi::flow_context_set_floatspace(p, match *no_gamma_correction { true => ::ffi::Floatspace::srgb, _ => ::ffi::Floatspace::linear},0f32,0f32,0f32)

            }

            if let Some(s::Build001Config{ graph_recording, ..}) = parsed.builder_config {
                if let Some(r) = graph_recording {
                    job.configure_graph_recording(r);
                }
            }


            IoTranslator::new(p).add_to_job(job.as_ptr(), parsed.io);

            ::flow::execution_engine::Engine::create(&mut job, &mut g).execute()?;

            // TODO: flow_job_destroy

            // TODO: Question, should JSON endpoints populate the Context error stacktrace when something goes wrong? Or populate both (except for OOM).


            let mut encodes = Vec::new();
            for node in g.raw_nodes(){
                if let ::flow::definitions::NodeResult::Encoded(ref r) = node.weight.result{
                    encodes.push((*r).clone());
                }
            }
            Ok(s::ResponsePayload::BuildResult(s::JobResult{encodes:encodes}))
        }
    }
}

// #[test]
fn test_handler() {

    let input_io = s::IoObject {
        io_id: 0,
        direction: s::IoDirection::Input,

        io: s::IoEnum::BytesHex("FFD8FFE000104A46494600010101004800480000FFDB004300FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC2000B080001000101011100FFC40014100100000000000000000000000000000000FFDA0008010100013F10".to_owned())
    };

    let output_io = s::IoObject {
        io_id: 1,
        direction: s::IoDirection::Output,

        io: s::IoEnum::OutputBuffer,
    };

    let mut steps = vec![];
    steps.push(s::Node::Decode { io_id: 0, commands: None });
    steps.push(s::Node::Resample2D {
        w: 20,
        h: 30,
        down_filter: None,
        up_filter: None,
        hints: None
    });
    steps.push(s::Node::FlipV);
    steps.push(s::Node::FlipH);
    steps.push(s::Node::Rotate90);
    steps.push(s::Node::Rotate180);
    steps.push(s::Node::Rotate270);
    steps.push(s::Node::Transpose);
    steps.push(s::Node::ExpandCanvas {
        top: 2,
        left: 3,
        bottom: 4,
        right: 5,
        color: s::Color::Srgb(s::ColorSrgb::Hex("aeae22".to_owned())),
    });
    steps.push(s::Node::FillRect {
        x1: 0,
        x2: 10,
        y1: 0,
        y2: 10,
        color: s::Color::Srgb(s::ColorSrgb::Hex("ffee00".to_owned())),
    });
    steps.push(s::Node::Encode {
        io_id: 1,
        preset: s::EncoderPreset::LibjpegTurbo{ quality: Some(90)}
    });

    let build = s::Build001 {
        builder_config: Some(s::Build001Config {
            graph_recording: None,
            process_all_gif_frames: Some(false),
            enable_jpeg_block_scaling: Some(false),
            no_gamma_correction: false,
        }),
        io: vec![input_io, output_io],
        framewise: s::Framewise::Steps(steps),
    };

    let handler = Build_1_Handler::new();
    let response = handler.build_1(build);
}