//! Real-time voice pipeline: one actor task per session, bounded channels
//! between stages (each with a declared overflow policy), turn-taking, barge-in
//! via cancellation, plan execution and per-stage latency budgets.
//!
//! It talks to models and transports only through `vak-providers` ports and is
//! tested deterministically with paused time and scripted fakes.
