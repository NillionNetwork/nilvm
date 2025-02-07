# E2E Tests

This crate contains a number of functional tests and utilities to support testing NADA language features. 

## Tutorial - Adding new tests
The main test that you want to change is `test_nada_lang_type_feature`. You will want to add a new case(s) there.

Test cases use two parameters: 
- `type_name` - Correspond to one of the NADA types
- `template_id` - Identifier of the template in the [`templates`](templates/) folder
