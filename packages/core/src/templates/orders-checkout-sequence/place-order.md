---
type: "uml.Sequence"
title: "Place Order"
describes: [Order](./order.md)
---

# Place Order

## Lifelines
- [Customer](./customer.md)
- [Order](./order.md) as order
- [PricingService](./pricing-service.md) as pricing

## Messages
- Customer calls order: `place(items)`
- order calls pricing: `calculateTotal(items)`
- pricing replies order: `total`
- alt
  - when `paymentAuthorized`
    - order calls order: `recordOrder()`
    - order replies Customer: `confirmation`
  - else
    - order sends Customer: `paymentFailed()`
