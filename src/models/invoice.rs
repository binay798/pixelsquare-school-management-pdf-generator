//! Typed invoice payload accepted by `POST /api/v1/invoices`.
//!
//! The payload is validated and converted into the JSON "view model" that the
//! `templates/invoice.typ` template consumes. All arithmetic (line amounts,
//! taxes, totals) happens here so the template only has to render values.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::models::format::{DATE_FORMAT, format_date, format_money, format_quantity, round2};

/// A company or person that appears on the invoice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Party {
    /// Display name (required, must not be empty).
    pub name: String,
    /// Address lines, rendered one per line.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub address: Vec<String>,
    /// Contact e-mail address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Contact phone number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    /// VAT / tax identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tax_id: Option<String>,
}

/// A single billed line item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceItem {
    /// Description of the goods or services.
    pub description: String,
    /// Billed quantity, defaults to `1`.
    #[serde(default = "default_quantity")]
    pub quantity: f64,
    /// Unit of measure (e.g. `pcs`, `hour`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Price per unit.
    pub unit_price: f64,
    /// Tax rate for this line in **percent**, e.g. `18.0` means 18%.
    #[serde(default)]
    pub tax_rate: f64,
}

fn default_quantity() -> f64 {
    1.0
}

/// Full invoice payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoicePayload {
    /// Human readable invoice number, e.g. `INV-2026-0001`.
    pub invoice_number: String,
    /// Issue date in `YYYY-MM-DD` format.
    pub issue_date: String,
    /// Due date in `YYYY-MM-DD` format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// ISO 4217 currency code, defaults to `USD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// Payment status shown as a badge, defaults to `unpaid`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Purchase order reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub po_number: Option<String>,
    /// Issuer of the invoice.
    pub seller: Party,
    /// Recipient of the invoice.
    pub buyer: Party,
    /// Billed line items (at least one is required).
    pub items: Vec<InvoiceItem>,
    /// Absolute discount amount subtracted from the subtotal.
    #[serde(default)]
    pub discount: f64,
    /// Absolute shipping/handling amount added to the total.
    #[serde(default)]
    pub shipping: f64,
    /// Free text notes rendered at the bottom.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Payment instructions rendered next to the totals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_terms: Option<String>,
}

impl InvoicePayload {
    /// Checks the business rules of the payload.
    ///
    /// Returns every problem that was found so the caller can report them all
    /// at once instead of failing on the first issue.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.invoice_number.trim().is_empty() {
            errors.push("`invoice_number` must not be empty".to_string());
        }

        if NaiveDate::parse_from_str(&self.issue_date, DATE_FORMAT).is_err() {
            errors.push(format!(
                "`issue_date` must be a valid date in `{DATE_FORMAT}` format, got `{}`",
                self.issue_date
            ));
        }

        if let Some(due_date) = &self.due_date
            && NaiveDate::parse_from_str(due_date, DATE_FORMAT).is_err()
        {
            errors.push(format!(
                "`due_date` must be a valid date in `{DATE_FORMAT}` format, got `{due_date}`"
            ));
        }

        if self.seller.name.trim().is_empty() {
            errors.push("`seller.name` must not be empty".to_string());
        }

        if self.buyer.name.trim().is_empty() {
            errors.push("`buyer.name` must not be empty".to_string());
        }

        if self.items.is_empty() {
            errors.push("`items` must contain at least one line item".to_string());
        }

        for (index, item) in self.items.iter().enumerate() {
            let position = index + 1;

            if item.description.trim().is_empty() {
                errors.push(format!(
                    "`items[{index}].description` (line {position}) must not be empty"
                ));
            }

            if !item.quantity.is_finite() || item.quantity <= 0.0 {
                errors.push(format!(
                    "`items[{index}].quantity` (line {position}) must be greater than 0"
                ));
            }

            if !item.unit_price.is_finite() || item.unit_price < 0.0 {
                errors.push(format!(
                    "`items[{index}].unit_price` (line {position}) must not be negative"
                ));
            }

            if !item.tax_rate.is_finite() || !(0.0..=100.0).contains(&item.tax_rate) {
                errors.push(format!(
                    "`items[{index}].tax_rate` (line {position}) must be between 0 and 100"
                ));
            }
        }

        if !self.discount.is_finite() || self.discount < 0.0 {
            errors.push("`discount` must not be negative".to_string());
        }

        if !self.shipping.is_finite() || self.shipping < 0.0 {
            errors.push("`shipping` must not be negative".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validates the payload and builds the JSON view model for the template.
    pub fn to_template_data(&self) -> Result<Value, Vec<String>> {
        self.validate()?;

        let currency = self
            .currency
            .as_deref()
            .unwrap_or(DEFAULT_CURRENCY)
            .to_ascii_uppercase();

        let mut subtotal = 0.0;
        let mut tax_total = 0.0;
        let mut items = Vec::with_capacity(self.items.len());

        for (index, item) in self.items.iter().enumerate() {
            let amount = round2(item.quantity * item.unit_price);
            let tax_amount = round2(amount * item.tax_rate / 100.0);

            subtotal += amount;
            tax_total += tax_amount;

            items.push(json!({
                "index": index + 1,
                "description": item.description,
                "quantity": item.quantity,
                "quantity_display": format_quantity(item.quantity),
                "unit": item.unit.clone().unwrap_or_default(),
                "unit_price": item.unit_price,
                "unit_price_display": format_money(item.unit_price),
                "amount": amount,
                "amount_display": format_money(amount),
                "tax_rate": item.tax_rate,
                "tax_amount": tax_amount,
                "tax_amount_display": format_money(tax_amount),
            }));
        }

        let subtotal = round2(subtotal);
        let tax_total = round2(tax_total);
        let discount = round2(self.discount);
        let shipping = round2(self.shipping);
        let total = round2(subtotal - discount + shipping + tax_total);

        let status = self
            .status
            .as_deref()
            .unwrap_or(DEFAULT_STATUS)
            .to_ascii_lowercase();

        let due_date = match &self.due_date {
            Some(value) => json!({ "raw": value, "display": format_date(value) }),
            None => Value::Null,
        };

        Ok(json!({
            "invoice_number": self.invoice_number.trim(),
            "issue_date": {
                "raw": self.issue_date,
                "display": format_date(&self.issue_date),
            },
            "due_date": due_date,
            "currency": currency,
            "currency_symbol": currency_symbol(&currency),
            "status": status,
            "is_paid": status == "paid",
            "po_number": self.po_number.clone().unwrap_or_default(),
            "seller": {
                "name": self.seller.name.trim(),
                "address": self.seller.address,
                "email": self.seller.email.clone().unwrap_or_default(),
                "phone": self.seller.phone.clone().unwrap_or_default(),
                "tax_id": self.seller.tax_id.clone().unwrap_or_default(),
            },
            "buyer": {
                "name": self.buyer.name.trim(),
                "address": self.buyer.address,
                "email": self.buyer.email.clone().unwrap_or_default(),
                "phone": self.buyer.phone.clone().unwrap_or_default(),
                "tax_id": self.buyer.tax_id.clone().unwrap_or_default(),
            },
            "items": items,
            "item_count": self.items.len(),
            "subtotal": subtotal,
            "subtotal_display": format_money(subtotal),
            "discount": discount,
            "discount_display": format_money(discount),
            "has_discount": discount > 0.0,
            "shipping": shipping,
            "shipping_display": format_money(shipping),
            "has_shipping": shipping > 0.0,
            "tax_total": tax_total,
            "tax_total_display": format_money(tax_total),
            "has_tax": tax_total > 0.0,
            "total": total,
            "total_display": format_money(total),
            "notes": self.notes.clone().unwrap_or_default(),
            "payment_terms": self.payment_terms.clone().unwrap_or_default(),
        }))
    }
}

const DEFAULT_CURRENCY: &str = "USD";
const DEFAULT_STATUS: &str = "unpaid";

/// Returns the symbol of a currency code (empty string when unknown).
fn currency_symbol(currency: &str) -> &'static str {
    match currency {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "INR" => "₹",
        "JPY" => "¥",
        "AUD" => "A$",
        "CAD" => "C$",
        "SGD" => "S$",
        "CHF" => "CHF",
        "AED" => "AED",
        "NPR" => "रू",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_payload() -> InvoicePayload {
        InvoicePayload {
            invoice_number: "INV-2026-0001".to_string(),
            issue_date: "2026-11-08".to_string(),
            due_date: Some("2026-12-08".to_string()),
            currency: Some("usd".to_string()),
            status: None,
            po_number: Some("PO-42".to_string()),
            seller: Party {
                name: "Pixelsquare".to_string(),
                address: vec!["Kathmandu".to_string()],
                email: Some("billing@pixelsquare.io".to_string()),
                phone: None,
                tax_id: None,
            },
            buyer: Party {
                name: "Springfield School".to_string(),
                address: vec!["Main Street 1".to_string()],
                email: None,
                phone: None,
                tax_id: None,
            },
            items: vec![
                InvoiceItem {
                    description: "Tuition fee".to_string(),
                    quantity: 2.0,
                    unit: Some("month".to_string()),
                    unit_price: 1500.0,
                    tax_rate: 10.0,
                },
                InvoiceItem {
                    description: "Lab fee".to_string(),
                    quantity: 1.0,
                    unit: None,
                    unit_price: 250.5,
                    tax_rate: 0.0,
                },
            ],
            discount: 100.0,
            shipping: 0.0,
            notes: Some("Thank you".to_string()),
            payment_terms: None,
        }
    }

    #[test]
    fn computes_totals() {
        let payload = sample_payload();
        let data = payload.to_template_data().expect("payload is valid");

        // 2 * 1500 = 3000 (+10% tax = 300), 1 * 250.5 = 250.5
        assert_eq!(data["subtotal"], json!(3250.5));
        assert_eq!(data["tax_total"], json!(300.0));
        assert_eq!(data["discount"], json!(100.0));
        assert_eq!(data["total"], json!(3450.5));
        assert_eq!(data["currency"], json!("USD"));
        assert_eq!(data["currency_symbol"], json!("$"));
        assert_eq!(data["total_display"], json!("3,450.50"));
        assert_eq!(data["items"][0]["amount_display"], json!("3,000.00"));
        assert_eq!(data["items"][0]["quantity_display"], json!("2"));
    }

    #[test]
    fn formats_dates_and_defaults() {
        let payload = sample_payload();
        let data = payload.to_template_data().expect("payload is valid");

        assert_eq!(data["issue_date"]["display"], json!("08 Nov 2026"));
        assert_eq!(data["due_date"]["display"], json!("08 Dec 2026"));
        assert_eq!(data["status"], json!("unpaid"));
        assert_eq!(data["is_paid"], json!(false));
    }

    #[test]
    fn rejects_invalid_payloads() {
        let mut payload = sample_payload();
        payload.invoice_number = "  ".to_string();
        payload.issue_date = "08/11/2026".to_string();
        payload.items.clear();

        let errors = payload.validate().expect_err("payload is invalid");
        assert_eq!(errors.len(), 3, "unexpected errors: {errors:?}");
    }

    #[test]
    fn rejects_bad_line_items() {
        let mut payload = sample_payload();
        payload.items[0].quantity = 0.0;
        payload.items[1].tax_rate = 150.0;

        let errors = payload.validate().expect_err("payload is invalid");
        assert_eq!(errors.len(), 2, "unexpected errors: {errors:?}");
    }

    #[test]
    fn formats_money_values() {
        assert_eq!(format_money(0.0), "0.00");
        assert_eq!(format_money(1234567.891), "1,234,567.89");
        assert_eq!(format_money(-42.5), "-42.50");
        assert_eq!(format_quantity(3.0), "3");
        assert_eq!(format_quantity(2.5), "2.5");
    }
}
