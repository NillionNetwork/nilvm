# Automatic tests for NADA

Testing Utility for NADA language that:
- Generates programs for every operation and for every single supported combination of operands
- Runs tests for every program using our program simulator (we don't need a local network of nodes but we validate all protocols)
- Validates results using bytecode evaluator
- Generates a JUnit report for visibility

