pub mod logger;
pub mod task;
pub mod workflow;

pub use logger::{LogLevel, Logger, ProgressReporter};

#[cfg(test)]
pub mod test {
    use crate::{
        task::{
            DataType,
            schema::{InputSpec, OutputSpec},
        },
        workflow::{TriggerDefinition, TriggerSchema, result::TriggerResult},
    };

    #[test]
    fn test_all() {
        use uuid::Uuid;

        use crate::{
            task::{InputBinding, Task, TaskDefinition, TaskResult, TaskSchema, Value},
            workflow::{Trigger, Workflow},
        };

        let file_added_trigger = TriggerDefinition::builder()
            .name("file_added")
            .triggered(|_ctx| TriggerResult::default())
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
            .execute(|_ctx| TaskResult::default())
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
            .execute(|_ctx| TaskResult::default())
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
            .execute(|_ctx| TaskResult::default())
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
            .execute(|_ctx| TaskResult::default())
            .build()
            .unwrap();

        // Create a workflow
        let mut wf = Workflow::builder()
            .name("Image Processing Workflow".to_string())
            .description(Some("Workflow to process and upload images".to_string()))
            .build()
            .unwrap();

        // Add a trigger (workflow owns it)
        let new_file_trigger = Trigger::builder()
            .id(Uuid::new_v4())
            .name("new_file")
            .definition(&file_added_trigger)
            .input(("path", Value::String("/Downloads")))
            .build()
            .unwrap();

        // Add tasks through the workflow
        let resize = Task::builder()
            .name("resize_image")
            .definition(&resize_image)
            .input((
                "file",
                InputBinding::trigger(new_file_trigger.id(), "file_path"),
            ))
            .input(("width", InputBinding::literal(Value::Integer(1920))))
            .input(("height", InputBinding::literal(Value::Integer(1080))))
            .dependency(new_file_trigger.id())
            .build()
            .unwrap();

        let compress = Task::builder()
            .name("compress_image")
            .definition(&compress_image)
            .input(("file", InputBinding::task(resize.id(), "resized_file")))
            .retry(2u32)
            .build()
            .unwrap();

        let upload = Task::builder()
            .name("upload_file")
            .definition(&upload_file)
            .input(("file", InputBinding::task(compress.id(), "compressed_file")))
            .input(("bucket", InputBinding::literal(Value::String("my-bucket"))))
            .build()
            .unwrap();

        let thumbnail = Task::builder()
            .name("generate_thumbnail")
            .definition(&generate_thumbnail)
            .input(("file", InputBinding::task(resize.id(), "resized_file")))
            .parallel(true)
            .build()
            .unwrap();

        wf.add_trigger(new_file_trigger);
        wf.add_task(resize);
        wf.add_task(compress);
        wf.add_task(upload);
        wf.add_task(thumbnail);
    }
}
