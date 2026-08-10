---
type: uml.StateMachineDiagram
title: Sign In
---

# Sign In

## Nodes

### SignedOut
- on `password` transitions to SignedIn traces [AUTH-PASSWORD](./sign-in-contract.md#auth-password)
- on `authenticated` transitions to SignedIn
  traces [AUTH-OIDC-004](./sign-in-contract.md#auth-oidc-004)
  traces [OIDC Core](https://openid.net/specs/openid-connect-core-1_0.html)

### SignedIn
- on `signout` transitions to SignedOut
