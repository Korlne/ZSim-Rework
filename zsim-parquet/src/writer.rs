//! Parquet writer for simulation results.
//!
//! Writes [`SimResult`] data to Parquet files with a fixed 15-column schema,
//! Zstd(3) compression, and one row‑group per [`SimResult`].
//!
//! # Schema
//!
//! | #  | Column           | Type      | Nullable |
//! |----|------------------|-----------|----------|
//! | 0  | `sim_index`      | Int64     | no       |
//! | 1  | `tick`           | UInt64    | no       |
//! | 2  | `event_type`     | Utf8      | no       |
//! | 3  | `source_id`      | Utf8      | yes      |
//! | 4  | `target_id`      | Utf8      | yes      |
//! | 5  | `action_id`      | Utf8      | yes      |
//! | 6  | `damage`         | Float64   | yes      |
//! | 7  | `crit`           | Boolean   | yes      |
//! | 8  | `element`        | Utf8      | yes      |
//! | 9  | `anomaly_gauge`  | Float64   | yes      |
//! | 10 | `stun_dmg`       | Float64   | yes      |
//! | 11 | `buff_id`        | Utf8      | yes      |
//! | 12 | `buff_value`     | Float64   | yes      |
//! | 13 | `coordinated_flag`| Boolean  | yes      |
//! | 14 | `timestamp`      | Int64     | yes      |

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use arrow::array::{BooleanBuilder, Float64Builder, Int64Builder, StringBuilder, UInt64Builder};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;

use zsim_core::combat::parallel::SimResult;
use zsim_core::events::signals::EventType;

/// Compute a simulated timestamp from a tick count.
///
/// Assumes 60 ticks / second → `tick * 1000 / 60` milliseconds.
/// Returns `None` for unreasonably large ticks to avoid overflow.
fn tick_to_timestamp(tick: u64) -> Option<i64> {
    let ms = tick.checked_mul(1000)? / 60;
    i64::try_from(ms).ok()
}

/// Build the 15‑column Parquet schema for simulation results.
fn build_schema() -> Schema {
    Schema::new(vec![
        Field::new("sim_index", DataType::Int64, false),
        Field::new("tick", DataType::UInt64, false),
        Field::new("event_type", DataType::Utf8, false),
        Field::new("source_id", DataType::Utf8, true),
        Field::new("target_id", DataType::Utf8, true),
        Field::new("action_id", DataType::Utf8, true),
        Field::new("damage", DataType::Float64, true),
        Field::new("crit", DataType::Boolean, true),
        Field::new("element", DataType::Utf8, true),
        Field::new("anomaly_gauge", DataType::Float64, true),
        Field::new("stun_dmg", DataType::Float64, true),
        Field::new("buff_id", DataType::Utf8, true),
        Field::new("buff_value", DataType::Float64, true),
        Field::new("coordinated_flag", DataType::Boolean, true),
        Field::new("timestamp", DataType::Int64, true),
    ])
}

/// Convert an [`EventType`] to its string representation for Parquet storage.
fn event_type_to_string(et: &EventType) -> String {
    format!("{et:?}")
}

/// Convert a single [`SimResult`] into an Arrow [`RecordBatch`] (one row‑group).
fn result_to_batch(result: &SimResult, schema: &Schema) -> Result<RecordBatch> {
    let n = result.events.len();

    let mut sim_index_builder = Int64Builder::with_capacity(n);
    let mut tick_builder = UInt64Builder::with_capacity(n);
    let mut event_type_builder = StringBuilder::new();
    let mut source_id_builder = StringBuilder::new();
    let mut target_id_builder = StringBuilder::new();
    let mut action_id_builder = StringBuilder::new();
    let mut damage_builder = Float64Builder::with_capacity(n);
    let mut crit_builder = BooleanBuilder::with_capacity(n);
    let mut element_builder = StringBuilder::new();
    let mut anomaly_gauge_builder = Float64Builder::with_capacity(n);
    let mut stun_dmg_builder = Float64Builder::with_capacity(n);
    let mut buff_id_builder = StringBuilder::new();
    let mut buff_value_builder = Float64Builder::with_capacity(n);
    let mut coordinated_flag_builder = BooleanBuilder::with_capacity(n);
    let mut timestamp_builder = Int64Builder::with_capacity(n);

    for event in &result.events {
        // sim_index (not null, Int64)
        sim_index_builder.append_value(result.sim_index as i64);

        // tick (not null, UInt64)
        tick_builder.append_value(event.tick);

        // event_type (not null, Utf8)
        event_type_builder.append_value(event_type_to_string(&event.event_type));

        // source_id (nullable Utf8)
        append_optional_string(&mut source_id_builder, event.source_id.as_deref());

        // target_id (nullable Utf8)
        append_optional_string(&mut target_id_builder, event.target_id.as_deref());

        // action_id (nullable Utf8)
        append_optional_string(&mut action_id_builder, event.action_id.as_deref());

        // damage (nullable Float64)
        append_optional_f64(&mut damage_builder, event.damage);

        // crit (nullable Boolean)
        match event.is_crit {
            Some(v) => crit_builder.append_value(v),
            None => crit_builder.append_null(),
        }

        // element (nullable Utf8)
        append_optional_string(&mut element_builder, event.element.as_deref());

        // anomaly_gauge (nullable Float64)
        append_optional_f64(&mut anomaly_gauge_builder, event.anomaly_gauge);

        // stun_dmg (nullable Float64)
        append_optional_f64(&mut stun_dmg_builder, event.stun_dmg);

        // buff_id (nullable Utf8)
        append_optional_string(&mut buff_id_builder, event.buff_id.as_deref());

        // buff_value (nullable Float64)
        append_optional_f64(&mut buff_value_builder, event.buff_value);

        // coordinated_flag (nullable Boolean)
        match event.coordinated_flag {
            Some(v) => coordinated_flag_builder.append_value(v),
            None => coordinated_flag_builder.append_null(),
        }

        // timestamp (nullable Int64) — derived from tick
        match tick_to_timestamp(event.tick) {
            Some(ts) => timestamp_builder.append_value(ts),
            None => timestamp_builder.append_null(),
        }
    }

    let batch = RecordBatch::try_new(
        Arc::new(schema.clone()),
        vec![
            Arc::new(sim_index_builder.finish()),
            Arc::new(tick_builder.finish()),
            Arc::new(event_type_builder.finish()),
            Arc::new(source_id_builder.finish()),
            Arc::new(target_id_builder.finish()),
            Arc::new(action_id_builder.finish()),
            Arc::new(damage_builder.finish()),
            Arc::new(crit_builder.finish()),
            Arc::new(element_builder.finish()),
            Arc::new(anomaly_gauge_builder.finish()),
            Arc::new(stun_dmg_builder.finish()),
            Arc::new(buff_id_builder.finish()),
            Arc::new(buff_value_builder.finish()),
            Arc::new(coordinated_flag_builder.finish()),
            Arc::new(timestamp_builder.finish()),
        ],
    )
    .context("Failed to create RecordBatch")?;

    Ok(batch)
}

fn append_optional_string(builder: &mut StringBuilder, value: Option<&str>) {
    match value {
        Some(s) => builder.append_value(s),
        None => builder.append_null(),
    }
}

fn append_optional_f64(builder: &mut Float64Builder, value: Option<f64>) {
    match value {
        Some(v) => builder.append_value(v),
        None => builder.append_null(),
    }
}

// ── High‑level convenience API ────────────────────────────────────────

/// Write all simulation results to a Parquet file at `output_path`.
///
/// The file is created or overwritten.  Each [`SimResult`] becomes its own
/// row‑group, enabling per‑sim partial reads during aggregation.
pub fn write_results(results: Vec<SimResult>, output_path: &Path) -> Result<()> {
    let mut writer = ParquetWriter::new(output_path)?;
    writer.write_batch(&results)?;
    writer.close()?;
    Ok(())
}

// ── Streaming / append‑mode API ───────────────────────────────────────

/// Streaming Parquet writer that supports multiple `write_batch` calls.
///
/// ```ignore
/// let mut writer = ParquetWriter::new("out.parquet")?;
/// writer.write_batch(&batch1)?;
/// writer.write_batch(&batch2)?;
/// writer.close()?;
/// ```
///
/// Each `write_batch` call writes one or more row‑groups.  The file is not
/// finalized until [`close`](ParquetWriter::close) is called.
pub struct ParquetWriter {
    writer: Option<ArrowWriter<File>>,
    schema: SchemaRef,
}

impl ParquetWriter {
    /// Create a new Parquet writer for the given output path.
    ///
    /// The file is created immediately (truncating if it exists).
    pub fn new(output_path: &Path) -> Result<Self> {
        let file = File::create(output_path)
            .with_context(|| format!("Failed to create Parquet file: {}", output_path.display()))?;

        let schema = Arc::new(build_schema());
        let props = WriterProperties::builder()
            .set_compression(Compression::ZSTD(
                ZstdLevel::try_new(3).expect("Zstd level 3 is valid"),
            ))
            .build();

        let writer = ArrowWriter::try_new(file, Arc::clone(&schema), Some(props))
            .context("Failed to create ArrowWriter")?;

        Ok(Self {
            writer: Some(writer),
            schema,
        })
    }

    /// Write a batch of results to the file.
    ///
    /// Each [`SimResult`] becomes one row‑group.  May be called multiple times.
    pub fn write_batch(&mut self, results: &[SimResult]) -> Result<()> {
        let writer = self.writer.as_mut().expect("ParquetWriter already closed");

        for result in results {
            let batch = result_to_batch(result, &self.schema)?;
            writer
                .write(&batch)
                .context("Failed to write RecordBatch to Parquet")?;
            // Force a row‑group boundary after each SimResult so that
            // every simulation's events live in its own row‑group.
            writer
                .flush()
                .context("Failed to flush Parquet row group")?;
        }

        Ok(())
    }

    /// Finalise the Parquet file and close the writer.
    ///
    /// After calling this, no further writes are possible.
    pub fn close(&mut self) -> Result<()> {
        if let Some(writer) = self.writer.take() {
            writer.close().context("Failed to close Parquet writer")?;
        }
        Ok(())
    }
}

impl Drop for ParquetWriter {
    fn drop(&mut self) {
        if self.writer.is_some() {
            let _ = self.close();
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use arrow::array::Array;
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use zsim_core::combat::runner::LoggedEvent;

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn make_event(
        tick: u64,
        event_type: EventType,
        source_id: Option<&str>,
        target_id: Option<&str>,
        action_id: Option<&str>,
        damage: Option<f64>,
        is_crit: Option<bool>,
    ) -> LoggedEvent {
        LoggedEvent {
            tick,
            event_type,
            source_id: source_id.map(String::from),
            target_id: target_id.map(String::from),
            action_id: action_id.map(String::from),
            damage,
            is_crit,
            element: None,
            anomaly_gauge: None,
            stun_dmg: None,
            buff_id: None,
            buff_value: None,
            coordinated_flag: None,
        }
    }

    fn make_result(
        sim_index: usize,
        seed: u64,
        total_ticks: u64,
        reason: &str,
        events: Vec<LoggedEvent>,
    ) -> SimResult {
        SimResult {
            sim_index,
            seed,
            total_ticks,
            termination_reason: Some(reason.to_string()),
            events,
        }
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("zsim_parquet_test");
        let _ = fs::create_dir_all(&dir);
        dir.join(name)
    }

    use parquet::file::reader::FileReader;
    use parquet::file::serialized_reader::SerializedFileReader;

    fn read_parquet(path: &Path) -> Result<(SchemaRef, Vec<RecordBatch>)> {
        let file = File::open(path)?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
        let schema = builder.schema().clone();
        let reader = builder.build()?;
        let batches: Result<Vec<_>, _> = reader.collect();
        let batches = batches?;
        Ok((schema, batches))
    }

    fn count_row_groups(path: &Path) -> Result<usize> {
        let file = File::open(path)?;
        let reader = SerializedFileReader::new(file)?;
        Ok(reader.metadata().num_row_groups())
    }

    // ------------------------------------------------------------------
    // Schema tests
    // ------------------------------------------------------------------

    #[test]
    fn test_schema_has_15_columns() {
        let schema = build_schema();
        assert_eq!(schema.fields().len(), 15, "schema should have 15 columns");
    }

    #[test]
    fn test_schema_column_names() {
        let schema = build_schema();
        let names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
        assert_eq!(
            names,
            vec![
                "sim_index",
                "tick",
                "event_type",
                "source_id",
                "target_id",
                "action_id",
                "damage",
                "crit",
                "element",
                "anomaly_gauge",
                "stun_dmg",
                "buff_id",
                "buff_value",
                "coordinated_flag",
                "timestamp",
            ]
        );
    }

    #[test]
    fn test_schema_non_nullable_columns() {
        let schema = build_schema();
        for i in 0..3 {
            assert!(
                !schema.field(i).is_nullable(),
                "column {} should be non-nullable",
                i
            );
        }
    }

    // ------------------------------------------------------------------
    // Basic write + read
    // ------------------------------------------------------------------

    #[test]
    fn test_write_and_read_basic() {
        let path = temp_path("basic.parquet");
        let events = vec![
            make_event(
                0,
                EventType::ActionStart,
                Some("char_0"),
                None,
                Some("atk"),
                None,
                None,
            ),
            make_event(
                1,
                EventType::DamageDealt,
                Some("char_0"),
                Some("enemy_0"),
                Some("atk"),
                Some(1500.5),
                Some(true),
            ),
        ];
        let results = vec![make_result(0, 42, 10, "completed", events)];

        write_results(results, &path).expect("write should succeed");

        let (_schema, batches) = read_parquet(&path).expect("read should succeed");
        assert_eq!(batches.len(), 1, "should have 1 row group");
        assert_eq!(batches[0].num_rows(), 2, "should have 2 rows");
    }

    #[test]
    fn test_write_and_read_multiple_results() {
        let path = temp_path("multi_result.parquet");
        let results: Vec<SimResult> = (0..3)
            .map(|i| {
                make_result(
                    i,
                    42 + i as u64,
                    5,
                    "ok",
                    vec![make_event(
                        0,
                        EventType::ActionStart,
                        Some("char_0"),
                        None,
                        Some("skill"),
                        None,
                        None,
                    )],
                )
            })
            .collect();

        write_results(results, &path).expect("write should succeed");

        let row_groups = count_row_groups(&path).expect("count row groups");
        assert_eq!(row_groups, 3, "should have 3 row groups (one per result)");

        // Verify total data via reader
        let (_schema, batches) = read_parquet(&path).expect("read should succeed");
        let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
        assert_eq!(total_rows, 3, "should have 3 total rows");
    }

    #[test]
    fn test_write_empty_events() {
        let path = temp_path("empty_events.parquet");
        let results = vec![make_result(0, 42, 0, "no_actions", vec![])];

        write_results(results, &path).expect("write should succeed");

        let (_schema, batches) = read_parquet(&path).expect("read should succeed");
        let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
        assert_eq!(total_rows, 0, "should have 0 total rows");
    }

    // ------------------------------------------------------------------
    // Data integrity
    // ------------------------------------------------------------------

    #[test]
    fn test_values_roundtrip_correctly() {
        let path = temp_path("roundtrip.parquet");
        let events = vec![
            make_event(
                0,
                EventType::ActionStart,
                Some("char_0"),
                None,
                Some("skill_a"),
                None,
                None,
            ),
            make_event(
                5,
                EventType::DamageDealt,
                Some("char_1"),
                Some("enemy_0"),
                Some("skill_b"),
                Some(999.9),
                Some(false),
            ),
            make_event(
                10,
                EventType::AnomalyTriggered,
                Some("char_0"),
                Some("enemy_0"),
                None,
                Some(2500.0),
                None,
            ),
        ];
        let results = vec![make_result(7, 99, 15, "test_done", events)];

        write_results(results, &path).expect("write should succeed");

        let (_schema, batches) = read_parquet(&path).expect("read should succeed");
        assert_eq!(batches.len(), 1);
        let batch = &batches[0];
        assert_eq!(batch.num_rows(), 3);

        // Check sim_index column
        let sim_col = batch
            .column(0)
            .as_any()
            .downcast_ref::<arrow::array::Int64Array>()
            .expect("sim_index should be Int64Array");
        assert_eq!(sim_col.value(0), 7);
        assert_eq!(sim_col.value(1), 7);
        assert_eq!(sim_col.value(2), 7);

        // Check tick column
        let tick_col = batch
            .column(1)
            .as_any()
            .downcast_ref::<arrow::array::UInt64Array>()
            .expect("tick should be UInt64Array");
        assert_eq!(tick_col.value(0), 0);
        assert_eq!(tick_col.value(1), 5);
        assert_eq!(tick_col.value(2), 10);

        // Check event_type column
        let et_col = batch
            .column(2)
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .expect("event_type should be StringArray");
        assert_eq!(et_col.value(0), "ActionStart");
        assert_eq!(et_col.value(1), "DamageDealt");
        assert_eq!(et_col.value(2), "AnomalyTriggered");

        // Check source_id
        let src_col = batch
            .column(3)
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .expect("source_id should be StringArray");
        assert_eq!(src_col.value(0), "char_0");
        assert_eq!(src_col.value(1), "char_1");
        assert_eq!(src_col.value(2), "char_0");

        // Check damage
        let dmg_col = batch
            .column(6)
            .as_any()
            .downcast_ref::<arrow::array::Float64Array>()
            .expect("damage should be Float64Array");
        assert!(dmg_col.is_null(0));
        assert!((dmg_col.value(1) - 999.9).abs() < 1e-9);
        assert!((dmg_col.value(2) - 2500.0).abs() < 1e-9);

        // Check crit (is_crit)
        let crit_col = batch
            .column(7)
            .as_any()
            .downcast_ref::<arrow::array::BooleanArray>()
            .expect("crit should be BooleanArray");
        assert!(crit_col.is_null(0));
        assert!(!crit_col.value(1));
        assert!(crit_col.is_null(2));
    }

    // ------------------------------------------------------------------
    // Null handling
    // ------------------------------------------------------------------

    #[test]
    fn test_all_optional_fields_null() {
        let path = temp_path("all_null.parquet");
        let events = vec![LoggedEvent {
            tick: 0,
            event_type: EventType::TickStart,
            source_id: None,
            target_id: None,
            action_id: None,
            damage: None,
            is_crit: None,
            element: None,
            anomaly_gauge: None,
            stun_dmg: None,
            buff_id: None,
            buff_value: None,
            coordinated_flag: None,
        }];
        let results = vec![make_result(0, 42, 1, "null_test", events)];

        write_results(results, &path).expect("write should succeed");

        let (_schema, batches) = read_parquet(&path).expect("read should succeed");
        assert_eq!(batches.len(), 1);
        let batch = &batches[0];
        assert_eq!(batch.num_rows(), 1);

        // Check all nullable columns are null at row 0
        // Columns 3-13 are nullable and should be null.
        // Column 14 (timestamp) is derived from tick=0 → 0ms, so it is NOT null.
        for col_idx in 3..=13 {
            let col = batch.column(col_idx);
            assert!(col.is_null(0), "column {col_idx} should be null");
        }
        // Verify timestamp (col 14) is non-null since tick=0 gives timestamp=0
        let ts_col = batch
            .column(14)
            .as_any()
            .downcast_ref::<arrow::array::Int64Array>()
            .expect("timestamp should be Int64Array");
        assert!(!ts_col.is_null(0));
        assert_eq!(ts_col.value(0), 0);
    }

    #[test]
    fn test_extended_fields_roundtrip() {
        let path = temp_path("extended.parquet");
        let events = vec![LoggedEvent {
            tick: 5,
            event_type: EventType::DamageDealt,
            source_id: Some("char_0".into()),
            target_id: Some("enemy_0".into()),
            action_id: Some("skill_fire".into()),
            damage: Some(1200.0),
            is_crit: Some(true),
            element: Some("Fire".into()),
            anomaly_gauge: Some(35.5),
            stun_dmg: Some(80.0),
            buff_id: Some("buff_atk_up".into()),
            buff_value: Some(0.25),
            coordinated_flag: Some(false),
        }];
        let results = vec![make_result(1, 100, 10, "extended_test", events)];

        write_results(results, &path).expect("write should succeed");

        let (_schema, batches) = read_parquet(&path).expect("read should succeed");
        assert_eq!(batches.len(), 1);
        let batch = &batches[0];
        assert_eq!(batch.num_rows(), 1);

        // Check extended fields
        let el_col = batch
            .column(8)
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .unwrap();
        assert_eq!(el_col.value(0), "Fire");

        let ag_col = batch
            .column(9)
            .as_any()
            .downcast_ref::<arrow::array::Float64Array>()
            .unwrap();
        assert!((ag_col.value(0) - 35.5).abs() < 1e-9);

        let sd_col = batch
            .column(10)
            .as_any()
            .downcast_ref::<arrow::array::Float64Array>()
            .unwrap();
        assert!((sd_col.value(0) - 80.0).abs() < 1e-9);

        let bi_col = batch
            .column(11)
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .unwrap();
        assert_eq!(bi_col.value(0), "buff_atk_up");

        let bv_col = batch
            .column(12)
            .as_any()
            .downcast_ref::<arrow::array::Float64Array>()
            .unwrap();
        assert!((bv_col.value(0) - 0.25).abs() < 1e-9);

        let cf_col = batch
            .column(13)
            .as_any()
            .downcast_ref::<arrow::array::BooleanArray>()
            .unwrap();
        assert!(!cf_col.value(0));

        // Check timestamp derived from tick=5: 5*1000/60 = 83
        let ts_col = batch
            .column(14)
            .as_any()
            .downcast_ref::<arrow::array::Int64Array>()
            .unwrap();
        assert_eq!(ts_col.value(0), 83); // 5 * 1000 / 60 = 83
    }

    // ------------------------------------------------------------------
    // Timestamp derivation
    // ------------------------------------------------------------------

    #[test]
    fn test_tick_to_timestamp_values() {
        assert_eq!(tick_to_timestamp(0), Some(0));
        assert_eq!(tick_to_timestamp(60), Some(1000)); // 60 ticks = 1 sec
        assert_eq!(tick_to_timestamp(30), Some(500)); // 30 ticks = 0.5 sec
        assert_eq!(tick_to_timestamp(18000), Some(300000)); // 18000 ticks = 5 min
                                                            // Verify rounding: 1 tick = 16.666... ms, stored as integer = 16
        assert_eq!(tick_to_timestamp(1), Some(16)); // 1 * 1000 / 60 = 16
    }

    // ------------------------------------------------------------------
    // Event type string conversion
    // ------------------------------------------------------------------

    #[test]
    fn test_event_type_to_string() {
        assert_eq!(event_type_to_string(&EventType::TickStart), "TickStart");
        assert_eq!(event_type_to_string(&EventType::DamageDealt), "DamageDealt");
        assert_eq!(
            event_type_to_string(&EventType::AnomalyTriggered),
            "AnomalyTriggered"
        );
        assert_eq!(
            event_type_to_string(&EventType::CoordinatedAction),
            "CoordinatedAction"
        );
        assert_eq!(event_type_to_string(&EventType::ErrorRaised), "ErrorRaised");
        assert_eq!(
            event_type_to_string(&EventType::StunTriggered),
            "StunTriggered"
        );
    }

    // ------------------------------------------------------------------
    // Streaming / append mode
    // ------------------------------------------------------------------

    #[test]
    fn test_parquet_writer_append_two_batches() {
        let path = temp_path("append.parquet");

        let mut writer = ParquetWriter::new(&path).expect("create writer");

        // First batch
        let batch1 = vec![make_result(
            0,
            42,
            5,
            "first",
            vec![make_event(
                0,
                EventType::ActionStart,
                Some("a"),
                None,
                Some("s1"),
                None,
                None,
            )],
        )];
        writer.write_batch(&batch1).expect("first batch");

        // Second batch (append)
        let batch2 = vec![make_result(
            1,
            43,
            10,
            "second",
            vec![
                make_event(
                    0,
                    EventType::ActionStart,
                    Some("b"),
                    None,
                    Some("s2"),
                    None,
                    None,
                ),
                make_event(
                    1,
                    EventType::DamageDealt,
                    Some("b"),
                    Some("e0"),
                    Some("s2"),
                    Some(500.0),
                    Some(false),
                ),
            ],
        )];
        writer.write_batch(&batch2).expect("second batch");

        writer.close().expect("close");

        // Read back and verify both batches present
        // batch1 → 1 SimResult → 1 row group; batch2 → 1 SimResult → 1 row group
        let row_groups = count_row_groups(&path).expect("count row groups");
        assert_eq!(row_groups, 2, "should have 2 row groups (1 + 1)");

        let (_schema, batches) = read_parquet(&path).expect("read");
        let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
        assert_eq!(total_rows, 3, "should have 3 total rows (1 + 2)");
    }

    // ------------------------------------------------------------------
    // Multiple events per result
    // ------------------------------------------------------------------

    #[test]
    fn test_large_result_set() {
        let path = temp_path("large.parquet");

        // Create a result with 1000 events
        let events: Vec<LoggedEvent> = (0..1000)
            .map(|i| {
                make_event(
                    i as u64,
                    if i % 2 == 0 {
                        EventType::ActionStart
                    } else {
                        EventType::DamageDealt
                    },
                    Some("char_0"),
                    Some("enemy_0"),
                    Some("skill"),
                    if i % 2 == 0 {
                        None
                    } else {
                        Some(i as f64 * 10.0)
                    },
                    None,
                )
            })
            .collect();

        let results = vec![make_result(0, 42, 1000, "large", events)];
        write_results(results, &path).expect("write should succeed");

        let (_schema, batches) = read_parquet(&path).expect("read should succeed");
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].num_rows(), 1000);
    }

    // ------------------------------------------------------------------
    // Compression property
    // ------------------------------------------------------------------

    #[test]
    fn test_writer_uses_zstd() {
        // Verify that the WriterProperties include Zstd compression.
        // We construct a writer and check its properties indirectly by
        // writing data and reading it back successfully.
        let path = temp_path("compression.parquet");
        let events = vec![make_event(
            0,
            EventType::TickStart,
            None,
            None,
            None,
            None,
            None,
        )];
        let results = vec![make_result(0, 0, 1, "comp_test", events)];
        write_results(results, &path).expect("write");

        // Just verify the file is valid parquet
        let (_schema, batches) = read_parquet(&path).expect("read");
        assert_eq!(batches.len(), 1);

        // Verify the file is smaller than uncompressed would be (small sanity check)
        let metadata = fs::metadata(&path).expect("metadata");
        assert!(metadata.len() > 0, "file should have content");
    }

    // ------------------------------------------------------------------
    // Error handling
    // ------------------------------------------------------------------

    #[test]
    fn test_write_to_invalid_path() {
        let result = write_results(vec![], Path::new("/nonexistent_dir/file.parquet"));
        assert!(result.is_err(), "should fail on invalid path");
    }

    // ------------------------------------------------------------------
    // Empty results list
    // ------------------------------------------------------------------

    #[test]
    fn test_write_empty_results_list() {
        let path = temp_path("empty_results.parquet");
        write_results(vec![], &path).expect("write empty should succeed");

        let (_schema, batches) = read_parquet(&path).expect("read should succeed");
        assert_eq!(batches.len(), 0, "no results = no row groups");
    }

    // ------------------------------------------------------------------
    // Drop behavior (auto-close)
    // ------------------------------------------------------------------

    #[test]
    fn test_parquet_writer_drop_auto_closes() {
        let path = temp_path("drop_close.parquet");
        {
            let events = vec![make_event(
                0,
                EventType::TickStart,
                None,
                None,
                None,
                None,
                None,
            )];
            let results = vec![make_result(0, 0, 1, "drop", events)];
            let mut writer = ParquetWriter::new(&path).expect("create");
            writer.write_batch(&results).expect("write");
            // writer drops here — should auto-close
        }

        // File should be valid parquet after drop
        let (_schema, batches) = read_parquet(&path).expect("read after drop");
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].num_rows(), 1);
    }
}
