---
type: uml.Class
title: Invoice
---
# Invoice

## Attributes
- id: InvoiceId {1}
- total: Decimal {1}

## Relationships
- associates [Payment](./payment.md): 1 invoice to 0..* payments
