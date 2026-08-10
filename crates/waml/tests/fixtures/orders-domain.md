<!-- waml/1 part 7d91ac42f5e649c4a6cd939cfa60b920 shop/order.md -->
---
type: uml.Class
stereotype: [aggregateRoot, entity]
title: Order
description: "A customer's placed order."
---
# Order

## Attributes
- id: OrderId {1}
- status: [OrderStatus](./order-status.md) {1}
- total: [Money](./money.md) {1}

## Relationships
- associates [Customer](./customer.md): 1 order to 1 customer
- composes [OrderLine](./order-line.md): 1 to 1..* lines

<!-- waml/1 part 7d91ac42f5e649c4a6cd939cfa60b920 shop/order-line.md -->
---
type: uml.Class
stereotype: entity
title: OrderLine
---
# OrderLine

## Attributes
- quantity: Int {1}
- unitPrice: [Money](./money.md) {1}

<!-- waml/1 part 7d91ac42f5e649c4a6cd939cfa60b920 shop/customer.md -->
---
type: uml.Class
stereotype: [aggregateRoot, entity]
title: Customer
---
# Customer

## Attributes
- id: CustomerId {1}
- name: String {1}

<!-- waml/1 part 7d91ac42f5e649c4a6cd939cfa60b920 shop/order-status.md -->
---
type: uml.Enum
title: OrderStatus
---
# OrderStatus

## Values
- DRAFT
- PLACED
- SHIPPED
- CANCELLED

<!-- waml/1 part 7d91ac42f5e649c4a6cd939cfa60b920 shop/money.md -->
---
type: uml.DataType
stereotype: valueObject
title: Money
---
# Money

## Attributes
- amount: Decimal {1}
- currency: CurrencyCode {1}

<!-- waml/1 part 7d91ac42f5e649c4a6cd939cfa60b920 shop/orders-domain.md -->
---
type: uml.ClassDiagram
title: Orders Domain
profile: uml-domain
---
# Orders Domain

## Members
- [Order](./order.md)
- [OrderLine](./order-line.md)
- [Customer](./customer.md)
- [OrderStatus](./order-status.md)
- [Money](./money.md)
