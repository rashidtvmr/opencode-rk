# Context checkpoint observer collision correction

Independent observer correction for the frozen test artifact SHA-256
`20f5ba5a58efe90457f7c273f47133f3adc4a72faae6e3e93c36f8a6b4b16cec`.

The final replacement-checkpoint assertion failed only because
`opaque-secret-native-window` is a prefix of
`opaque-secret-native-window-second`; substring absence therefore falsely
reported the old window as present. The semantic intent is unchanged: the
new opaque marker must be present as the exact `encrypted_content` value and
the old marker must be absent as that exact value. Provider payload assertions
now recursively inspect JSON `encrypted_content` fields, while UI secrecy
assertions remain broad string absence checks. The source failure was recorded
at `docs/worklogs/context-controls-004/context-selection-backend-final-check.log`
line 402. This is an observer supersession/correction, not PASS evidence;
parent reruns the frozen test and records the result.
