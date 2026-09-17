# Example corpus

This catalog is a map of the Fidryn example tree. The modules under
`examples/` and the independent programs under `tests/programs/` are
research fixtures. They are not legal advice, not operative instruments,
and not a complete statement of any jurisdiction's law.

Build the toolchain as in [getting-started](getting-started.md). Source
syntax is in [language](language.md). Subcommands are in [cli](cli.md).

## How to use a state fixture

State modules under `examples/states/` do not ship companion JSON. Case
facts live in `scenario` blocks inside the `.fr` file. Digests are
`fixture`. Mechanical clocks and caps are `automatic`; open-textured
standards use `Determine` / `Observe`.

```
fidryn check examples/states/florida/homestead.fr
fidryn verify examples/states/florida/homestead.fr --property HomesteadHonesty
```

`verify` honesty properties often report `unknown` under the v0.1
Boolean-literal prover when asserts are English-shaped. That is
intentional (see [language](language.md) Verify). Distinguish `unknown`
from `counterexample` (a real failed Boolean assert).

The quality bar is `examples/states/florida/homestead.fr`.

## Fifty-state corpus

| State path | Module |
| --- | --- |
| `examples/states/alabama/final-wages.fr` | `Alabama.Employment.FinalWages` |
| `examples/states/alaska/security-deposit.fr` | `Alaska.Housing.SecurityDeposit` |
| `examples/states/arizona/material-noncompliance.fr` | `Arizona.Housing.MaterialNoncomplianceNotice` |
| `examples/states/arkansas/homestead.fr` | `Arkansas.Property.Homestead` |
| `examples/states/california/wage-statements.fr` | `California.Employment.WageStatements` |
| `examples/states/colorado/famli.fr` | `Colorado.Employment.FAMLI` |
| `examples/states/connecticut/paid-sick-leave.fr` | `Connecticut.Employment.PaidSickLeave` |
| `examples/states/delaware/wage-payment.fr` | `Delaware.Employment.WagePayment` |
| `examples/states/florida/homestead.fr` | `Florida.Property.Homestead` |
| `examples/states/georgia/dispossessory.fr` | `Georgia.Housing.Dispossessory` |
| `examples/states/hawaii/security-deposit.fr` | `Hawaii.Housing.SecurityDeposit` |
| `examples/states/idaho/noncompete.fr` | `Idaho.Employment.Noncompete` |
| `examples/states/illinois/wage-payment.fr` | `Illinois.Employment.WagePayment` |
| `examples/states/indiana/security-deposit.fr` | `Indiana.Housing.SecurityDeposit` |
| `examples/states/iowa/security-deposit.fr` | `Iowa.Housing.SecurityDeposit` |
| `examples/states/kansas/homestead.fr` | `Kansas.Property.Homestead` |
| `examples/states/kentucky/final-wages.fr` | `Kentucky.Employment.FinalWages` |
| `examples/states/louisiana/community-property.fr` | `Louisiana.Family.CommunityProperty` |
| `examples/states/maine/paid-family-leave.fr` | `Maine.Employment.PaidFamilyLeave` |
| `examples/states/maryland/wage-payment.fr` | `Maryland.Employment.WagePayment` |
| `examples/states/massachusetts/pfml.fr` | `Massachusetts.Employment.PFML` |
| `examples/states/michigan/no-fault-pip.fr` | `Michigan.Insurance.NoFaultPIP` |
| `examples/states/minnesota/security-deposit.fr` | `Minnesota.Housing.SecurityDeposit` |
| `examples/states/mississippi/eviction.fr` | `Mississippi.Housing.Eviction` |
| `examples/states/missouri/security-deposit.fr` | `Missouri.Housing.SecurityDeposit` |
| `examples/states/montana/wrongful-discharge.fr` | `Montana.Employment.WrongfulDischarge` |
| `examples/states/nebraska/wage-payment.fr` | `Nebraska.Employment.WagePayment` |
| `examples/states/nevada/security-deposit.fr` | `Nevada.Housing.SecurityDeposit` |
| `examples/states/new-hampshire/wage-payment.fr` | `NewHampshire.Employment.WagePayment` |
| `examples/states/new-jersey/security-deposit.fr` | `NewJersey.Housing.SecurityDeposit` |
| `examples/states/new-mexico/owner-resident.fr` | `NewMexico.Housing.OwnerResident` |
| `examples/states/new-york/security-deposit-trust.fr` | `NewYork.Housing.SecurityDepositTrust` |
| `examples/states/north-carolina/security-deposit.fr` | `NorthCarolina.Housing.SecurityDeposit` |
| `examples/states/north-dakota/homestead.fr` | `NorthDakota.Property.Homestead` |
| `examples/states/ohio/security-deposit.fr` | `Ohio.Housing.SecurityDeposit` |
| `examples/states/oklahoma/wage-payment.fr` | `Oklahoma.Employment.WagePayment` |
| `examples/states/oregon/no-cause-notice.fr` | `Oregon.Housing.NoCauseNotice` |
| `examples/states/pennsylvania/security-deposit.fr` | `Pennsylvania.Housing.SecurityDeposit` |
| `examples/states/rhode-island/security-deposit.fr` | `RhodeIsland.Housing.SecurityDeposit` |
| `examples/states/south-carolina/residential-landlord.fr` | `SouthCarolina.Housing.ResidentialLandlord` |
| `examples/states/south-dakota/wage-payment.fr` | `SouthDakota.Employment.WagePayment` |
| `examples/states/tennessee/urlta.fr` | `Tennessee.Housing.URLTA` |
| `examples/states/texas/payday.fr` | `Texas.Employment.Payday` |
| `examples/states/utah/eviction.fr` | `Utah.Housing.Eviction` |
| `examples/states/vermont/security-deposit.fr` | `Vermont.Housing.SecurityDeposit` |
| `examples/states/virginia/vrlta-deposit.fr` | `Virginia.Housing.VRLTADeposit` |
| `examples/states/washington/paid-sick-leave.fr` | `Washington.Employment.PaidSickLeave` |
| `examples/states/west-virginia/final-wages.fr` | `WestVirginia.Employment.FinalWages` |
| `examples/states/wisconsin/security-deposit.fr` | `Wisconsin.Housing.SecurityDeposit` |
| `examples/states/wyoming/wage-payment.fr` | `Wyoming.Employment.WagePayment` |

## Other areas

| Area | Path |
| --- | --- |
| Trust | `examples/trust/` |
| Tax | `examples/tax/` |
| Federal slices | `examples/us-federal/` |
| Independent programs | `tests/programs/` (includes `transaction-atomic.fr`) |

A fuller narrative catalog may replace this map; paths above match the
tree on `dev` as of the 2026-09-17 states recheck (50/50 `fidryn check` ok).
