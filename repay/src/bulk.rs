//! Bulk operations: building their CSV uploads, and reading their outcome.
//!
//! ```
//! use repay::bulk::BulkAdjustment;
//! use repay::dec;
//! use repay::types::Amount;
//!
//! let mut adjustments = BulkAdjustment::new();
//! adjustments.row(1234, 987654, Amount::from(dec!(100)), Amount::from(dec!(25)));
//! let upload = adjustments.to_upload()?;
//! assert_eq!(
//!     String::from_utf8(upload.bytes).unwrap(),
//!     "MerchantId,TransactionId,OriginalAmount,AdjustmentAmount\n1234,987654,100,25\n"
//! );
//! # Ok::<(), repay::validate::ValidationError>(())
//! ```
//!
//! Send the upload with
//! `client.adjustments().bulk_disbursement_funding(&upload)`; the answer is a
//! [`BulkOutcome`].

use std::collections::HashSet;

use crate::error::Error;
use crate::protocol::decode_json;
use crate::transport::HttpResponse;
use crate::types::{Amount, Json, MerchantId, TransactionId, Upload};
use crate::validate::{Path, Validate, ValidationError, Violation, violation};

/// The rows of a bulk disbursement funding adjustment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BulkAdjustment {
    rows: Vec<AdjustmentRow>,
}

/// One adjustment: how much of a funding transaction to take back.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct AdjustmentRow {
    pub merchant_id: MerchantId,
    pub transaction_id: TransactionId,
    /// The transaction's original amount.
    pub original_amount: Amount,
    /// The amount to adjust, at most the original amount.
    pub adjustment_amount: Amount,
}

impl BulkAdjustment {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a row.
    pub fn row(
        &mut self,
        merchant_id: impl Into<MerchantId>,
        transaction_id: impl Into<TransactionId>,
        original_amount: Amount,
        adjustment_amount: Amount,
    ) -> &mut Self {
        self.rows.push(AdjustmentRow {
            merchant_id: merchant_id.into(),
            transaction_id: transaction_id.into(),
            original_amount,
            adjustment_amount,
        });
        self
    }

    pub fn rows(&self) -> &[AdjustmentRow] {
        &self.rows
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The CSV file to upload, after the checks the API applies first: at least
    /// one row, positive amounts, adjustments no larger than the original, and no
    /// duplicate rows.
    pub fn to_upload(&self) -> Result<Upload, ValidationError> {
        self.validate()?;
        let mut writer = csv::Writer::from_writer(Vec::new());
        let header = [
            "MerchantId",
            "TransactionId",
            "OriginalAmount",
            "AdjustmentAmount",
        ];
        let written = writer.write_record(header).and_then(|()| {
            self.rows.iter().try_for_each(|row| {
                writer.write_record([
                    row.merchant_id.to_string(),
                    row.transaction_id.to_string(),
                    row.original_amount.to_string(),
                    row.adjustment_amount.to_string(),
                ])
            })
        });
        written.expect("writing CSV to memory cannot fail");
        let bytes = writer.into_inner().expect("writing to memory cannot fail");
        Ok(Upload::csv("adjustments.csv", bytes))
    }
}

impl Validate for BulkAdjustment {
    fn validate_into(&self, path: &mut Path, out: &mut Vec<Violation>) {
        path.push_field("rows");
        if self.rows.is_empty() {
            violation(path, out, "must have at least 1 row".into());
        }
        let mut seen = HashSet::new();
        for (index, row) in self.rows.iter().enumerate() {
            path.push_index(index);
            for (name, amount) in [
                ("original_amount", row.original_amount),
                ("adjustment_amount", row.adjustment_amount),
            ] {
                path.push_field(name);
                if amount.as_decimal().is_sign_negative() || amount.as_decimal().is_zero() {
                    violation(path, out, format!("must be positive, not {amount}"));
                }
                amount.validate_into(path, out);
                path.pop();
            }
            if row.adjustment_amount > row.original_amount {
                path.push_field("adjustment_amount");
                violation(
                    path,
                    out,
                    format!(
                        "must be at most the original amount, {}",
                        row.original_amount
                    ),
                );
                path.pop();
            }
            if !seen.insert((row.merchant_id, row.transaction_id)) {
                violation(
                    path,
                    out,
                    format!("repeats transaction {}", row.transaction_id),
                );
            }
            path.pop();
        }
        path.pop();
    }
}

/// The transactions of a bulk card reversal.
//
// The spec names no column; `TransactionId` follows the adjustments file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BulkReversal {
    transactions: Vec<TransactionId>,
}

impl BulkReversal {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a transaction to reverse.
    pub fn transaction(&mut self, transaction_id: impl Into<TransactionId>) -> &mut Self {
        self.transactions.push(transaction_id.into());
        self
    }

    pub fn transactions(&self) -> &[TransactionId] {
        &self.transactions
    }

    /// The CSV file to upload: a `TransactionId` column.
    pub fn to_upload(&self) -> Upload {
        let mut csv = String::from("TransactionId\n");
        for id in &self.transactions {
            csv.push_str(&format!("{id}\n"));
        }
        Upload::csv("reversals.csv", csv)
    }
}

/// The answer to a bulk upload.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BulkOutcome {
    /// Every row was processed: the API's JSON result.
    Completed(Json),
    /// Some or all rows failed: the API's CSV of per-row results, with error
    /// messages.
    Failed(CsvReport),
}

/// A CSV file the API returned, such as a bulk operation's per-row results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvReport {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    raw: Vec<u8>,
}

impl CsvReport {
    /// Reads a CSV file whose first record is its header. Records may differ in
    /// length; text that is not UTF-8 is replaced.
    pub fn parse(bytes: impl Into<Vec<u8>>) -> Self {
        let raw = bytes.into();
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(raw.as_slice());
        let mut records = reader.byte_records().map_while(Result::ok).map(|record| {
            record
                .iter()
                .map(|field| String::from_utf8_lossy(field).into_owned())
                .collect::<Vec<_>>()
        });
        let headers = records.next().unwrap_or_default();
        let rows = records.collect();
        Self { headers, rows, raw }
    }

    /// The column names.
    pub fn headers(&self) -> &[String] {
        &self.headers
    }

    /// The records after the header.
    pub fn rows(&self) -> &[Vec<String>] {
        &self.rows
    }

    /// The index of the column `name`, ignoring case.
    pub fn column(&self, name: &str) -> Option<usize> {
        self.headers
            .iter()
            .position(|h| h.eq_ignore_ascii_case(name))
    }

    /// The value in row `row` (after the header) of the column `name`.
    pub fn get(&self, row: usize, name: &str) -> Option<&str> {
        let column = self.column(name)?;
        self.rows.get(row)?.get(column).map(String::as_str)
    }

    /// The file as received.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
}

/// Reads a bulk operation's successful (2xx) response: JSON, or a CSV report.
pub(crate) fn decode(response: &HttpResponse) -> Result<BulkOutcome, Error> {
    let json = response
        .content_type()
        .is_none_or(|ct| ct == "application/json" || ct == "text/json" || ct.ends_with("+json"));
    if json {
        decode_json(response).map(BulkOutcome::Completed)
    } else {
        Ok(BulkOutcome::Failed(CsvReport::parse(response.body.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dec;

    fn amount(value: rust_decimal::Decimal) -> Amount {
        Amount::from(value)
    }

    fn messages(adjustments: &BulkAdjustment) -> Vec<String> {
        match adjustments.validate() {
            Ok(()) => Vec::new(),
            Err(err) => err.violations().iter().map(ToString::to_string).collect(),
        }
    }

    #[test]
    fn writes_the_documented_columns() {
        let mut adjustments = BulkAdjustment::new();
        adjustments
            .row(1, 10, amount(dec!(100.00)), amount(dec!(25.50)))
            .row(1, 11, amount(dec!(5)), amount(dec!(5)));
        let upload = adjustments.to_upload().unwrap();
        assert_eq!(upload.filename, "adjustments.csv");
        assert_eq!(upload.content_type, "text/csv");
        assert_eq!(
            String::from_utf8(upload.bytes).unwrap(),
            "MerchantId,TransactionId,OriginalAmount,AdjustmentAmount\n1,10,100.00,25.50\n1,11,5,5\n"
        );
    }

    #[test]
    fn checks_rows_like_the_api() {
        assert_eq!(
            messages(&BulkAdjustment::new()),
            ["rows: must have at least 1 row"]
        );
        let mut adjustments = BulkAdjustment::new();
        adjustments
            .row(1, 10, amount(dec!(10)), amount(dec!(20)))
            .row(1, 11, amount(dec!(0)), amount(dec!(-1)))
            .row(1, 10, amount(dec!(10)), amount(dec!(1.005)));
        assert_eq!(
            messages(&adjustments),
            [
                "rows[0].adjustment_amount: must be at most the original amount, 10",
                "rows[1].original_amount: must be positive, not 0",
                "rows[1].adjustment_amount: must be positive, not -1",
                "rows[2].adjustment_amount: must have at most 2 decimal places, not 1.005",
                "rows[2]: repeats transaction 10",
            ]
        );
        assert!(adjustments.to_upload().is_err());
    }

    #[test]
    fn writes_reversals() {
        let mut reversals = BulkReversal::new();
        reversals.transaction(1001).transaction(1002);
        assert_eq!(
            String::from_utf8(reversals.to_upload().bytes).unwrap(),
            "TransactionId\n1001\n1002\n"
        );
    }

    #[test]
    fn reads_a_csv_report_with_quoted_fields() {
        let report = CsvReport::parse(
            "MerchantId,TransactionId,Status,Error\n\
             1,10,Failed,\"Adjustment exceeds the remaining balance, 4.00\"\n\
             1,11,Succeeded\n",
        );
        assert_eq!(
            report.headers(),
            ["MerchantId", "TransactionId", "Status", "Error"]
        );
        assert_eq!(report.rows().len(), 2);
        assert_eq!(
            report.get(0, "error"),
            Some("Adjustment exceeds the remaining balance, 4.00")
        );
        assert_eq!(report.get(1, "Error"), None);
        assert_eq!(report.get(1, "status"), Some("Succeeded"));
        assert!(report.raw().starts_with(b"MerchantId"));
    }

    #[test]
    fn decodes_json_or_csv() {
        let json = HttpResponse::new(
            200,
            vec![("Content-Type".into(), "application/json".into())],
            r#"{"processed": 2}"#,
        );
        let BulkOutcome::Completed(result) = decode(&json).unwrap() else {
            panic!("expected JSON");
        };
        assert_eq!(result.get("processed").and_then(|v| v.as_i64()), Some(2));

        let csv = HttpResponse::new(
            200,
            vec![("Content-Type".into(), "text/csv; charset=utf-8".into())],
            "TransactionId,Error\n10,Not found\n",
        );
        let BulkOutcome::Failed(report) = decode(&csv).unwrap() else {
            panic!("expected a CSV report");
        };
        assert_eq!(report.get(0, "Error"), Some("Not found"));
    }
}
