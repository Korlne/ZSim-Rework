use calamine::{open_workbook, DataType, Reader, Xlsx};
use rusqlite::{params, Connection};
use std::path::Path;

fn get_col(row: &[calamine::Data], idx: usize) -> String {
    row.get(idx)
        .and_then(|c| c.get_string())
        .unwrap_or("")
        .to_string()
}

fn get_num(row: &[calamine::Data], idx: usize) -> f64 {
    row.get(idx).and_then(|c| c.get_float()).unwrap_or(0.0)
}

fn get_int(row: &[calamine::Data], idx: usize) -> i64 {
    row.get(idx)
        .and_then(|c| c.get_float())
        .map(|f| f as i64)
        .unwrap_or(0)
}

pub fn import_characters_csv(conn: &Connection, file_path: &Path) -> Result<usize, String> {
    let path_str = file_path.to_string_lossy().to_string();
    let mut workbook: Xlsx<_> =
        open_workbook(&path_str).map_err(|e| format!("Failed to open {path_str}: {e}"))?;

    let range = workbook
        .worksheet_range_at(0)
        .ok_or_else(|| "No sheets in workbook".to_string())?
        .map_err(|e| format!("Failed to read sheet: {e}"))?;

    let mut rows = range.rows();
    if rows.next().is_none() {
        return Err("Empty spreadsheet".to_string());
    }

    let mut count = 0usize;
    for row in rows {
        let char_id = get_col(row, 0);
        if char_id.is_empty() {
            continue;
        }

        let name = get_col(row, 1);
        let faction = get_col(row, 2);
        let specialty = get_col(row, 3);
        let element = get_col(row, 4);
        let hp = get_num(row, 6);
        let atk = get_num(row, 7);
        let def = get_num(row, 8);
        let impact = get_num(row, 9);
        let crit_rate = get_num(row, 10);
        let crit_dmg = get_num(row, 11);
        let pen_ratio = get_num(row, 12);
        let pen_fixed = get_num(row, 13);
        let energy_regen = get_num(row, 14);
        let anomaly_mastery = get_num(row, 15);
        let anomaly_proficiency = get_num(row, 16);
        let energy_gen_rate = 0.0;

        conn.execute(
            "INSERT OR REPLACE INTO characters (char_id, name, faction, specialty, element,
                level, ascension, hp, atk, def, impact, crit_rate, crit_dmg, pen_ratio, pen_fixed,
                anomaly_mastery, anomaly_proficiency, energy_regen, energy_gen_rate,
                constellations, potentials, action_dict)
             VALUES (?1, ?2, ?3, ?4, ?5, 60, 6, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, '[false,false,false,false,false,false]',
                     '[false,false,false,false,false,false]', '[]')",
            params![
                char_id, name, faction, specialty, element,
                hp, atk, def, impact, crit_rate, crit_dmg,
                pen_ratio, pen_fixed, anomaly_mastery, anomaly_proficiency,
                energy_regen, energy_gen_rate,
            ],
        )
        .map_err(|e| format!("Failed to insert character '{char_id}': {e}"))?;
        count += 1;
    }

    Ok(count)
}

pub fn import_drive_disc_csv(conn: &Connection, file_path: &Path) -> Result<usize, String> {
    let path_str = file_path.to_string_lossy().to_string();
    let mut workbook: Xlsx<_> =
        open_workbook(&path_str).map_err(|e| format!("Failed to open {path_str}: {e}"))?;

    let range = workbook
        .worksheet_range_at(0)
        .ok_or_else(|| "No sheets in workbook".to_string())?
        .map_err(|e| format!("Failed to read sheet: {e}"))?;

    let mut rows = range.rows();
    if rows.next().is_none() {
        return Err("Empty spreadsheet".to_string());
    }

    let mut count = 0usize;
    for row in rows {
        let id = get_col(row, 0);
        if id.is_empty() {
            continue;
        }

        let set_id = get_col(row, 1);
        let slot = get_int(row, 2) as i32;
        let main_stat_name = get_col(row, 3);
        let main_stat_value = get_num(row, 4);
        let sub_stat_1_name: Option<String> = {
            let v = get_col(row, 5);
            if v.is_empty() { None } else { Some(v) }
        };
        let sub_stat_1_value: Option<f64> = {
            let v = get_num(row, 6);
            if v == 0.0 { None } else { Some(v) }
        };
        let sub_stat_2_name: Option<String> = {
            let v = get_col(row, 7);
            if v.is_empty() { None } else { Some(v) }
        };
        let sub_stat_2_value: Option<f64> = {
            let v = get_num(row, 8);
            if v == 0.0 { None } else { Some(v) }
        };

        conn.execute(
            "INSERT OR REPLACE INTO drive_discs (id, slot, level, set_id,
                main_stat_name, main_stat_value,
                sub_stat_1_name, sub_stat_1_value,
                sub_stat_2_name, sub_stat_2_value,
                sub_stat_3_name, sub_stat_3_value,
                sub_stat_4_name, sub_stat_4_value)
             VALUES (?1, ?2, 15, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, NULL, NULL, NULL)",
            params![
                id, slot, set_id,
                main_stat_name, main_stat_value,
                sub_stat_1_name, sub_stat_1_value,
                sub_stat_2_name, sub_stat_2_value,
            ],
        )
        .map_err(|e| format!("Failed to insert drive disc '{id}': {e}"))?;
        count += 1;
    }

    Ok(count)
}

pub fn import_w_engine_csv(conn: &Connection, file_path: &Path) -> Result<usize, String> {
    let path_str = file_path.to_string_lossy().to_string();
    let mut workbook: Xlsx<_> =
        open_workbook(&path_str).map_err(|e| format!("Failed to open {path_str}: {e}"))?;

    let range = workbook
        .worksheet_range_at(0)
        .ok_or_else(|| "No sheets in workbook".to_string())?
        .map_err(|e| format!("Failed to read sheet: {e}"))?;

    let mut rows = range.rows();
    if rows.next().is_none() {
        return Err("Empty spreadsheet".to_string());
    }

    let mut count = 0usize;
    for row in rows {
        let id = get_col(row, 0);
        if id.is_empty() {
            continue;
        }

        let name = get_col(row, 1);
        let atk = get_num(row, 3);
        let crit_rate = get_num(row, 4);
        let crit_dmg = get_num(row, 5);
        let pen_ratio = get_num(row, 6);
        let energy_regen = get_num(row, 7);
        let impact = get_num(row, 8);
        let anomaly_mastery = get_num(row, 9);

        conn.execute(
            "INSERT OR REPLACE INTO w_engines (id, name, level, ascension, atk, crit_rate, crit_dmg, pen_ratio, energy_regen, impact, anomaly_mastery, passive_effects)
             VALUES (?1, ?2, 60, 6, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '[]')",
            params![id, name, atk, crit_rate, crit_dmg, pen_ratio, energy_regen, impact, anomaly_mastery],
        )
        .map_err(|e| format!("Failed to insert w-engine '{id}': {e}"))?;
        count += 1;
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_entry::db::init_db;

    #[test]
    fn test_import_characters_csv_nonexistent_file() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();
        let result = import_characters_csv(&conn, Path::new("nonexistent.xlsx"));
        assert!(result.is_err());
    }
}
