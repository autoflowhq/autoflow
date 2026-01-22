pub mod compiler;
pub mod executor;
pub mod logger;
pub mod plugin;
pub mod task;
pub mod trigger;
pub mod workflow;

mod error;
mod value;
pub use error::{CoreError, Result};
pub use value::Value;

#[cfg(test)]
pub mod test {
    use std::collections::HashMap;

    use crate::{
        Value,
        executor::WorkflowExecutor,
        plugin::{Plugin, Registry},
        task::{DataType, InputSpec, OutputSpec, TaskContext, TaskHandler, TaskResult},
        trigger::{TriggerDefinition, TriggerResult, TriggerSchema},
        workflow::DependencyRef,
    };

    struct ResizeImageHandler;

    impl TaskHandler for ResizeImageHandler {
        fn execute<'a>(&self, _inputs: &TaskContext<'a>) -> TaskResult<'a> {
            let outputs = [("resized_file", Value::String("resized_image.jpg"))];
            TaskResult::builder().outputs(outputs).build().unwrap()
        }
    }

    struct CompressImageHandler;

    impl TaskHandler for CompressImageHandler {
        fn execute<'a>(&self, _inputs: &TaskContext<'a>) -> TaskResult<'a> {
            let outputs = [("compressed_file", Value::String("compressed_image.jpg"))];
            TaskResult::builder().outputs(outputs).build().unwrap()
        }
    }

    struct UploadFileHandler;

    impl TaskHandler for UploadFileHandler {
        fn execute<'a>(&self, _inputs: &TaskContext<'a>) -> TaskResult<'a> {
            TaskResult::default()
        }
    }

    struct GenerateThumbnailHandler;

    impl TaskHandler for GenerateThumbnailHandler {
        fn execute<'a>(&self, _inputs: &TaskContext<'a>) -> TaskResult<'a> {
            let outputs = [("thumbnail_file", Value::String("thumbnail_image.jpg"))];
            TaskResult::builder().outputs(outputs).build().unwrap()
        }
    }

    #[test]
    fn test_all() {
        use crate::{
            Value,
            task::{InputBinding, Task, TaskDefinition, TaskSchema},
            trigger::Trigger,
            workflow::{Workflow, WorkflowCompiler},
        };

        let file_added_trigger = TriggerDefinition::builder()
            .name("file_added")
            .schema(
                TriggerSchema::builder()
                    .output(("file_path", DataType::File))
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let resize_image = TaskDefinition::builder()
            .name("resize_image")
            .schema(
                TaskSchema::builder()
                    .input(("file", InputSpec::new(DataType::File)))
                    .input(("width", InputSpec::new(DataType::Integer)))
                    .input(("height", InputSpec::new(DataType::Integer)))
                    .output(("resized_file", OutputSpec::new(DataType::File)))
                    .build()
                    .unwrap(),
            )
            .handler(Box::new(ResizeImageHandler))
            .build()
            .unwrap();

        let compress_image = TaskDefinition::builder()
            .name("compress_image")
            .schema(
                TaskSchema::builder()
                    .input(("file", InputSpec::new(DataType::File)))
                    .output(("compressed_file", OutputSpec::new(DataType::File)))
                    .build()
                    .unwrap(),
            )
            .handler(Box::new(CompressImageHandler))
            .build()
            .unwrap();

        let upload_file = TaskDefinition::builder()
            .name("upload_file")
            .schema(
                TaskSchema::builder()
                    .input(("file", InputSpec::new(DataType::File)))
                    .input(("bucket", InputSpec::new(DataType::String)))
                    .build()
                    .unwrap(),
            )
            .handler(Box::new(UploadFileHandler))
            .build()
            .unwrap();

        let generate_thumbnail = TaskDefinition::builder()
            .name("generate_thumbnail")
            .schema(
                TaskSchema::builder()
                    .input(("file", InputSpec::new(DataType::File)))
                    .build()
                    .unwrap(),
            )
            .handler(Box::new(GenerateThumbnailHandler))
            .build()
            .unwrap();

        let plugin = Plugin::builder()
            .name("ImageProcessingPlugin")
            .task(resize_image)
            .task(compress_image)
            .task(upload_file)
            .task(generate_thumbnail)
            .trigger(file_added_trigger)
            .build()
            .unwrap();

        let mut registry = Registry::new();
        registry.register_plugin(plugin);

        // Create a workflow
        let mut wf = Workflow::builder()
            .name("Image Processing Workflow")
            .description(Some("Workflow to process and upload images"))
            .build()
            .unwrap();

        // Add a trigger (workflow owns it)
        let new_file_trigger = Trigger::builder()
            .name("new_file")
            .definition(
                registry
                    .get_plugin("ImageProcessingPlugin")
                    .unwrap()
                    .get_trigger("file_added")
                    .unwrap(),
            )
            .input(("path", Value::String("/Downloads")))
            .build()
            .unwrap();

        // Add tasks through the workflow
        let resize = Task::builder()
            .name("resize_image")
            .definition(
                registry
                    .get_plugin("ImageProcessingPlugin")
                    .unwrap()
                    .get_task("resize_image")
                    .unwrap(),
            )
            .input((
                "file",
                InputBinding::trigger(new_file_trigger.id(), "file_path"),
            ))
            .input(("width", InputBinding::literal(Value::Integer(1920))))
            .input(("height", InputBinding::literal(Value::Integer(1080))))
            .dependency(DependencyRef::from_trigger(&new_file_trigger))
            .build()
            .unwrap();

        let compress = Task::builder()
            .name("compress_image")
            .definition(
                registry
                    .get_plugin("ImageProcessingPlugin")
                    .unwrap()
                    .get_task("compress_image")
                    .unwrap(),
            )
            .input(("file", InputBinding::task(resize.id(), "resized_file")))
            .retry(2u32)
            .build()
            .unwrap();

        let upload = Task::builder()
            .name("upload_file")
            .definition(
                registry
                    .get_plugin("ImageProcessingPlugin")
                    .unwrap()
                    .get_task("upload_file")
                    .unwrap(),
            )
            .input(("file", InputBinding::task(compress.id(), "compressed_file")))
            .input(("bucket", InputBinding::literal(Value::String("my-bucket"))))
            .build()
            .unwrap();

        let thumbnail = Task::builder()
            .name("generate_thumbnail")
            .definition(
                registry
                    .get_plugin("ImageProcessingPlugin")
                    .unwrap()
                    .get_task("generate_thumbnail")
                    .unwrap(),
            )
            .input(("file", InputBinding::task(resize.id(), "resized_file")))
            .parallel(true)
            .build()
            .unwrap();

        let new_file_trigger_id = new_file_trigger.id();

        wf.add_trigger(new_file_trigger);
        wf.add_task(resize);
        wf.add_task(compress);
        wf.add_task(upload);
        wf.add_task(thumbnail);

        // Compile the workflow before execution
        let compiled_wf = WorkflowCompiler::compile(&wf).unwrap();

        let mut executor = WorkflowExecutor::builder()
            .workflow(&compiled_wf)
            .plugins(&registry)
            .build()
            .unwrap();
        let mut outputs = HashMap::new();
        outputs.insert("file_path", Value::String("/Downloads/image1.jpg"));
        let result = TriggerResult::builder().outputs(outputs).build().unwrap();
        executor.trigger(new_file_trigger_id, result).unwrap();
    }
}
