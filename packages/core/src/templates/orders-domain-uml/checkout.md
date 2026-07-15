---
type: "uml.Activity"
title: "Checkout"
description: "Cart to shipped order."
describes: [Order](./order.md)
---

# Checkout

## Nodes

### initial
- transitions to Add Items to Cart

### Add Items to Cart
- partition: Customer
- transitions to In Stock?

### decision In Stock?
- partition: System
- when `inStock` transitions to Reserve Stock
- else transitions to Notify Out Of Stock

### Reserve Stock
- partition: System
- transitions to Calculate Total carries [Order](./order.md)

### Calculate Total
- partition: System
- transitions to Payment Authorized? carries [Order](./order.md)

### decision Payment Authorized?
- partition: System
- when `paymentAuthorized` transitions to Place Order
- else transitions to Cancel Order

### Place Order
- partition: System
- entry: `recordOrder`
- transitions to Ship Order carries [Order](./order.md)

### Ship Order
- partition: System
- transitions to final carries [Order](./order.md)

### Notify Out Of Stock
- partition: Customer
- transitions to Cancel Order

### Cancel Order
- partition: System
- transitions to final

### final
