# health-time

Layer: **foundation**. It re-exports `jiff::Timestamp` and `SignedDuration`, and defines the `Clock` port with `SystemClock` and the test `FixedClock`. Domain and application code take a `Clock` instead of reading the system time (dependency-rules.md rule 6).
