use crate::detect::ColumnDef;
use crate::errors::Result;
use crate::model_traits::Column;
use crate::model_traits::{HasSchema, TableColumns, TableInfo};
use crate::writers::types::{are_equivalent_types, get_pairs, Pair};
use crate::Client;
use crate::Syntax;

mod issue;
pub use issue::*;

/// Returns a list of differences in the current database schema
/// and what the welds object was compiled against
///
/// Used to known if there are going to be issues when running the query of a model
pub async fn schema<T>(client: &dyn Client) -> Result<Vec<Issue>>
where
    T: Send + HasSchema,
    <T as HasSchema>::Schema: TableInfo + TableColumns,
{
    let mut problems = Vec::default();
    let identifier_parts: Vec<&str> = <T::Schema>::identifier().iter().rev().cloned().collect();
    let tablename = identifier_parts[0];
    let namespace = identifier_parts.get(1).copied();
    let namespace = unwrap_to_default_namespace(namespace, client);

    let tabledef = match crate::detect::find_table(namespace, tablename, client).await? {
        Some(x) => x,
        None => return Ok(vec![Issue::missing_table(namespace, tablename)]),
    };

    let table_cols = tabledef.columns();
    let model_cols = <T::Schema>::columns();
    let pairs = get_pairs(client.syntax());

    struct_added(table_cols, &model_cols)
        .iter()
        .for_each(|x| problems.push(Issue::struct_added(namespace, tablename, x)));

    build_diffs(&pairs, table_cols, &model_cols)
        .drain(..)
        .for_each(|x| problems.push(Issue::changed(namespace, tablename, x)));

    struct_missing(table_cols, &model_cols)
        .iter()
        .for_each(|x| problems.push(Issue::struct_missing(namespace, tablename, x)));

    Ok(problems)
}

/// returns the default namespace that is used if no namespace is provided
fn unwrap_to_default_namespace(
    ns: Option<&'static str>,
    client: &dyn Client,
) -> Option<&'static str> {
    if ns.is_some() {
        return ns;
    }
    let syntax = client.syntax();
    match syntax {
        Syntax::Mssql => Some("dbo"),
        Syntax::Postgres => Some("public"),
        // NOTE if schema is left out, the mysql query uses the name of the db in the connection
        Syntax::Mysql => None,
        Syntax::Sqlite => None,
    }
}

fn struct_missing<'a>(table_cols: &'a [ColumnDef], model_cols: &[Column]) -> Vec<&'a ColumnDef> {
    let model_has = |name: &str| model_cols.iter().any(|x| x.name() == name);
    table_cols
        .iter()
        .filter(|tc| !model_has(&tc.name))
        .collect()
}

fn struct_added<'a>(table_cols: &[ColumnDef], model_cols: &'a [Column]) -> Vec<&'a Column> {
    let table_has = |name: &str| table_cols.iter().any(|x| x.name == name);
    model_cols
        .iter()
        .filter(|mc| !table_has(mc.name()))
        .collect()
}

/// matches the rust models with the db models
fn zip_by_name<'a>(
    table_cols: &'a [ColumnDef],
    model_cols: &'a [Column],
) -> Vec<(&'a ColumnDef, &'a Column)> {
    let table_find = |name: &str| table_cols.iter().find(|x| x.name == name);
    model_cols
        .iter()
        .map(|mc| (table_find(mc.name()), mc))
        .filter(|x| x.0.is_some())
        .map(|x| (x.0.unwrap(), x.1))
        .collect()
}

/// returns true if this db column and model field do not line up
fn build_diff(pairs: &[Pair], dbcol: &ColumnDef, field: &Column) -> Option<Diff> {
    let type_changed = !are_equivalent_types(pairs, &dbcol.ty, field.rust_type());

    //eprintln!(
    //    "DB: {}\t\tR: {}\t{:?}",
    //    dbcol.ty,
    //    field.rust_type(),
    //    type_changed
    //);

    let nullable_chagned = dbcol.null != field.nullable();
    if type_changed || nullable_chagned {
        return Some(Diff {
            column: dbcol.name.to_string(),
            db_type: dbcol.ty.to_string(),
            db_nullable: dbcol.null,
            welds_type: field.rust_type().to_string(),
            welds_nullable: field.nullable(),
            type_changed,
        });
    }
    None
}

fn build_diffs<'a>(
    pairs: &[Pair],
    table_cols: &'a [ColumnDef],
    model_cols: &'a [Column],
) -> Vec<Diff> {
    zip_by_name(table_cols, model_cols)
        .into_iter()
        .filter_map(|(d, m)| build_diff(pairs, d, m))
        .collect()
}

///// Returns true if the two types are compatible
///// same_types("INT4", "INT4") == true
//fn same_types(t1: &str, t2: &str) -> bool {
//    if t1 == t2 {
//        return true;
//    }
//    if let Some(group) = find_same_group(t1) {
//        for x in group {
//            if t2 == *x {
//                return true;
//            }
//        }
//    }
//    false
//}
//
//fn find_same_group(t: &str) -> Option<&'static [&'static str]> {
//    for group in SAME_TYPES {
//        for inner in group.iter() {
//            if *inner == t {
//                return Some(group);
//            }
//        }
//    }
//    None
//}
//
//// list of all types that are compatible with each other.
//const SAME_TYPES: &[&[&str]] = &[
//    &["TEXT", "VARCHAR", "NVARCHAR"],
//    &["INT4", "INT", "SERIAL", "BIT", "NBIT"],
//    &["BIGINT", "INT8", "BIGSERIAL"],
//    &["BINYINT", "BOOLEAN"],
//    &["TINYINT", "BOOLEAN"],
//];
