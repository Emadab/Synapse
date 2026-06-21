//! Note-type editor commands: CRUD for note types, fields, and templates;
//! template preview rendered via `synapse-render`.

use synapse_core::ipc::{FieldRemoveWarning, IpcError, NotetypeDetail, RenderedPreview};
use synapse_core::Collection;
use synapse_render::{RenderRequest, Template};
use tauri::State;

type IpcResult<T> = Result<T, IpcError>;

#[tauri::command]
pub fn get_notetype(
    collection: State<'_, Collection>,
    notetype_id: i64,
) -> IpcResult<Option<NotetypeDetail>> {
    Ok(collection.get_notetype_detail(notetype_id)?)
}

#[tauri::command]
pub fn create_notetype(
    collection: State<'_, Collection>,
    name: String,
    kind: i64,
) -> IpcResult<NotetypeDetail> {
    Ok(collection.create_notetype(&name, kind)?)
}

#[tauri::command]
pub fn delete_notetype(collection: State<'_, Collection>, notetype_id: i64) -> IpcResult<()> {
    Ok(collection.delete_notetype(notetype_id)?)
}

#[tauri::command]
pub fn rename_notetype(
    collection: State<'_, Collection>,
    notetype_id: i64,
    name: String,
) -> IpcResult<()> {
    Ok(collection.rename_notetype(notetype_id, &name)?)
}

#[tauri::command]
pub fn add_field(
    collection: State<'_, Collection>,
    notetype_id: i64,
    name: String,
) -> IpcResult<()> {
    Ok(collection.add_field(notetype_id, &name)?)
}

#[tauri::command]
pub fn check_field_remove(
    collection: State<'_, Collection>,
    notetype_id: i64,
    ord: i64,
) -> IpcResult<FieldRemoveWarning> {
    Ok(collection.check_field_remove(notetype_id, ord)?)
}

#[tauri::command]
pub fn remove_field(
    collection: State<'_, Collection>,
    notetype_id: i64,
    ord: i64,
) -> IpcResult<()> {
    Ok(collection.remove_field(notetype_id, ord)?)
}

#[tauri::command]
pub fn rename_field(
    collection: State<'_, Collection>,
    notetype_id: i64,
    ord: i64,
    name: String,
) -> IpcResult<()> {
    Ok(collection.rename_field(notetype_id, ord, &name)?)
}

#[tauri::command]
pub fn reorder_fields(
    collection: State<'_, Collection>,
    notetype_id: i64,
    new_order: Vec<i64>,
) -> IpcResult<()> {
    Ok(collection.reorder_fields(notetype_id, new_order)?)
}

#[tauri::command]
pub fn add_template(
    collection: State<'_, Collection>,
    notetype_id: i64,
    name: String,
    qfmt: String,
    afmt: String,
) -> IpcResult<()> {
    Ok(collection.add_template(notetype_id, &name, &qfmt, &afmt)?)
}

#[tauri::command]
pub fn remove_template(
    collection: State<'_, Collection>,
    notetype_id: i64,
    ord: i64,
) -> IpcResult<()> {
    Ok(collection.remove_template(notetype_id, ord)?)
}

#[tauri::command]
pub fn save_template(
    collection: State<'_, Collection>,
    notetype_id: i64,
    ord: i64,
    name: String,
    qfmt: String,
    afmt: String,
) -> IpcResult<()> {
    Ok(collection.save_template(notetype_id, ord, &name, &qfmt, &afmt)?)
}

/// Render a template preview using the provided sample field values.
/// `sample_fields` is one value per field in ord order; empty strings are fine.
#[tauri::command]
pub fn preview_template(
    collection: State<'_, Collection>,
    notetype_id: i64,
    template_ord: i64,
    sample_fields: Vec<String>,
) -> IpcResult<RenderedPreview> {
    use synapse_core::error::CoreError;

    let detail = collection
        .get_notetype_detail(notetype_id)?
        .ok_or_else(|| CoreError::NotFound(format!("notetype {notetype_id}")))?;
    let tmpl = detail
        .templates
        .iter()
        .find(|t| t.ord == template_ord)
        .ok_or_else(|| CoreError::NotFound(format!("template {template_ord}")))?;
    let fields: Vec<(String, String)> = detail
        .fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            (
                f.name.clone(),
                sample_fields.get(i).cloned().unwrap_or_default(),
            )
        })
        .collect();
    let rendered = synapse_render::render(&RenderRequest {
        template: Template {
            qfmt: &tmpl.qfmt,
            afmt: &tmpl.afmt,
        },
        fields: &fields,
        card_ord: template_ord as u16,
        is_cloze: detail.kind == 1,
    });
    Ok(RenderedPreview {
        question: rendered.question,
        answer: rendered.answer,
    })
}
