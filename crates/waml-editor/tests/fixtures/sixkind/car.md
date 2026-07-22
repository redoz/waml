---
type: uml.Class
title: Car
---
# Car

## Attributes
- vin: String {1}

## Relationships
- specializes [Vehicle](./vehicle.md)
- implements [Drivable](./drivable.md)
- depends [Fuel](./fuel.md)
- associates [Driver](./driver.md): 1 to 1
- aggregates [Wheel](./wheel.md): 1 to *
- composes [Engine](./engine.md): 1 to 1
