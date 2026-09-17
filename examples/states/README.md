# Fifty-state high-impact corpus

One **full** Fidryn module per state: the statute or constitutional
rule people and lawyers actually hit, encoded as a complete instrument
(sources, entities, propositions, observations, rules, duties, queries,
scenarios, verification). These are research fixtures, not legal advice,
and not a complete statement of any state's code.

Case records live in the same `.fr` file as `scenario` blocks. There is
no companion JSON case file and no JSON source manifest.

`source_manifest` is omitted when the module's `source { ... }` block
already carries citation, artifact, and `digest fixture`. The compiler
loads an empty manifest in that case.

Mechanical clocks, caps, and acreage are `automatic`. Character,
willfulness, habitability, and similar open-textured questions stay in
`Determine` / `Observe`. Every module names what it leaves
`outside_scope`.

All fifty `fidryn check` clean on the 2026-09-17 snapshot.

## Signature doctrine per state

| State | File | Module | Principal citation |
| --- | --- | --- | --- |
| Alabama | `alabama/final-wages.fr` | `Alabama.Employment.FinalWages` | Ala. Code §§ 8-24-2 to 8-24-3 |
| Alaska | `alaska/security-deposit.fr` | `Alaska.Housing.SecurityDeposit` | AS 34.03.070 |
| Arizona | `arizona/material-noncompliance.fr` | `Arizona.Housing.MaterialNoncomplianceNotice` | A.R.S. § 33-1368 |
| Arkansas | `arkansas/homestead.fr` | `Arkansas.Property.Homestead` | Ark. Const. art. IX, §§ 3–5 |
| California | `california/wage-statements.fr` | `California.Employment.WageStatements` | Cal. Lab. Code § 226 |
| Colorado | `colorado/famli.fr` | `Colorado.Employment.FAMLI` | Colo. Rev. Stat. §§ 8-13.3-503 to 8-13.3-505 |
| Connecticut | `connecticut/paid-sick-leave.fr` | `Connecticut.Employment.PaidSickLeave` | Conn. Gen. Stat. §§ 31-57r, 31-57s |
| Delaware | `delaware/wage-payment.fr` | `Delaware.Employment.WagePayment` | 19 Del. C. §§ 1102, 1103 |
| Florida | `florida/homestead.fr` | `Florida.Property.Homestead` | Fla. Const. art. X, § 4 |
| Georgia | `georgia/dispossessory.fr` | `Georgia.Housing.Dispossessory` | O.C.G.A. § 44-7-50 et seq. |
| Hawaii | `hawaii/security-deposit.fr` | `Hawaii.Housing.SecurityDeposit` | Haw. Rev. Stat. § 521-44 |
| Idaho | `idaho/noncompete.fr` | `Idaho.Employment.Noncompete` | Idaho Code §§ 44-2701, 44-2704 |
| Illinois | `illinois/wage-payment.fr` | `Illinois.Employment.WagePayment` | 820 ILCS 115/3 to 115/14 |
| Indiana | `indiana/security-deposit.fr` | `Indiana.Housing.SecurityDeposit` | Ind. Code § 32-31-3 |
| Iowa | `iowa/security-deposit.fr` | `Iowa.Housing.SecurityDeposit` | Iowa Code § 562A.12 |
| Kansas | `kansas/homestead.fr` | `Kansas.Property.Homestead` | Kan. Const. art. 15, § 9 |
| Kentucky | `kentucky/final-wages.fr` | `Kentucky.Employment.FinalWages` | KRS 337.055 |
| Louisiana | `louisiana/community-property.fr` | `Louisiana.Family.CommunityProperty` | La. Civ. Code arts. 2338–2341 |
| Maine | `maine/paid-family-leave.fr` | `Maine.Employment.PaidFamilyLeave` | 26 M.R.S. §§ 850-A to 850-J |
| Maryland | `maryland/wage-payment.fr` | `Maryland.Employment.WagePayment` | Md. Code, Lab. & Empl. §§ 3-502, 3-505, 3-507.2 |
| Massachusetts | `massachusetts/pfml.fr` | `Massachusetts.Employment.PFML` | M.G.L. c. 175M §§ 2–6 |
| Michigan | `michigan/no-fault-pip.fr` | `Michigan.Insurance.NoFaultPIP` | MCL 500.3105, 500.3107 |
| Minnesota | `minnesota/security-deposit.fr` | `Minnesota.Housing.SecurityDeposit` | Minn. Stat. § 504B.178 |
| Mississippi | `mississippi/eviction.fr` | `Mississippi.Housing.Eviction` | Miss. Code Ann. §§ 89-8-13, 89-8-19, 89-8-39 |
| Missouri | `missouri/security-deposit.fr` | `Missouri.Housing.SecurityDeposit` | Mo. Rev. Stat. § 535.300 |
| Montana | `montana/wrongful-discharge.fr` | `Montana.Employment.WrongfulDischarge` | Mont. Code Ann. §§ 39-2-903 to 39-2-912 |
| Nebraska | `nebraska/wage-payment.fr` | `Nebraska.Employment.WagePayment` | Neb. Rev. Stat. §§ 48-1229 to 48-1234 |
| Nevada | `nevada/security-deposit.fr` | `Nevada.Housing.SecurityDeposit` | NRS 118A.240, 118A.242 |
| New Hampshire | `new-hampshire/wage-payment.fr` | `NewHampshire.Employment.WagePayment` | N.H. Rev. Stat. Ann. §§ 275:43, 275:44 |
| New Jersey | `new-jersey/security-deposit.fr` | `NewJersey.Housing.SecurityDeposit` | N.J.S.A. 46:8-21.1, 46:8-21.2 |
| New Mexico | `new-mexico/owner-resident.fr` | `NewMexico.Housing.OwnerResident` | NMSA 1978 §§ 47-8-18, 47-8-20, 47-8-37 |
| New York | `new-york/security-deposit-trust.fr` | `NewYork.Housing.SecurityDepositTrust` | N.Y. Gen. Oblig. Law §§ 7-103, 7-105 |
| North Carolina | `north-carolina/security-deposit.fr` | `NorthCarolina.Housing.SecurityDeposit` | N.C.G.S. §§ 42-50 to 42-55 |
| North Dakota | `north-dakota/homestead.fr` | `NorthDakota.Property.Homestead` | N.D. Const. art. XI, § 22 |
| Ohio | `ohio/security-deposit.fr` | `Ohio.Housing.SecurityDeposit` | Ohio Rev. Code § 5321.16 |
| Oklahoma | `oklahoma/wage-payment.fr` | `Oklahoma.Employment.WagePayment` | 40 O.S. §§ 165.2, 165.3 |
| Oregon | `oregon/no-cause-notice.fr` | `Oregon.Housing.NoCauseNotice` | ORS 90.427 |
| Pennsylvania | `pennsylvania/security-deposit.fr` | `Pennsylvania.Housing.SecurityDeposit` | 68 P.S. §§ 250.511a–250.512 |
| Rhode Island | `rhode-island/security-deposit.fr` | `RhodeIsland.Housing.SecurityDeposit` | R.I. Gen. Laws §§ 34-18-19, 34-18-24 |
| South Carolina | `south-carolina/residential-landlord.fr` | `SouthCarolina.Housing.ResidentialLandlord` | S.C. Code Ann. §§ 27-40-410, 27-40-710 |
| South Dakota | `south-dakota/wage-payment.fr` | `SouthDakota.Employment.WagePayment` | S.D. Codified Laws §§ 60-11-9, 60-11-10 |
| Tennessee | `tennessee/urlta.fr` | `Tennessee.Housing.URLTA` | Tenn. Code Ann. § 66-28-301 et seq. |
| Texas | `texas/payday.fr` | `Texas.Employment.Payday` | Tex. Lab. Code ch. 61 |
| Utah | `utah/eviction.fr` | `Utah.Housing.Eviction` | Utah Code §§ 78B-6-802, 78B-6-811 |
| Vermont | `vermont/security-deposit.fr` | `Vermont.Housing.SecurityDeposit` | 9 V.S.A. §§ 4451, 4461 |
| Virginia | `virginia/vrlta-deposit.fr` | `Virginia.Housing.VRLTADeposit` | Va. Code § 55.1-1226 |
| Washington | `washington/paid-sick-leave.fr` | `Washington.Employment.PaidSickLeave` | RCW 49.46.210 |
| West Virginia | `west-virginia/final-wages.fr` | `WestVirginia.Employment.FinalWages` | W. Va. Code § 21-5-4 |
| Wisconsin | `wisconsin/security-deposit.fr` | `Wisconsin.Housing.SecurityDeposit` | Wis. Stat. § 704.28; ATCP 134.06 |
| Wyoming | `wyoming/wage-payment.fr` | `Wyoming.Employment.WagePayment` | Wyo. Stat. § 27-4-104 |

The quality bar is `florida/homestead.fr`. Fixture excerpts live next to
each module under `sources/` and are not official editions.

District of Columbia and the territories are outside this corpus.
