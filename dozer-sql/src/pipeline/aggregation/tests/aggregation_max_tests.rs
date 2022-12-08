use crate::pipeline::aggregation::tests::aggregation_tests_utils::init_processor;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use dozer_core::{
    dag::{executor_local::DEFAULT_PORT_HANDLE, node::Processor},
    storage::transactions::SharedTransaction,
};
use dozer_types::rust_decimal::Decimal;
use dozer_types::types::DATE_FORMAT;
use dozer_types::{
    ordered_float::OrderedFloat,
    types::{Field, FieldDefinition, FieldType, Operation, Record, Schema},
};
use std::collections::HashMap;

#[test]
fn test_max_aggregation_float() {
    let (mut processor, tx) = init_processor(
        "SELECT Country, MAX(Salary) \
        FROM Users \
        WHERE Salary >= 1 GROUP BY Country",
    )
    .unwrap();

    let schema = Schema::empty()
        .field(
            FieldDefinition::new(String::from("ID"), FieldType::Int, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("Country"), FieldType::String, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("Salary"), FieldType::Float, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("MAX(Salary)"), FieldType::Float, false),
            false,
            false,
        )
        .clone();

    let _output_schema = processor
        .update_schema(
            DEFAULT_PORT_HANDLE,
            &HashMap::from([(DEFAULT_PORT_HANDLE, schema)]),
        )
        .unwrap();

    // Insert 100 for segment Italy
    /*
        Italy, 100.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(100.0)),
                Field::Float(OrderedFloat(100.0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(100.0)),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Insert another 100 for segment Italy
    /*
        Italy, 100.0
        Italy, 100.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(100.0)),
                Field::Float(OrderedFloat(100.0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(100.0)),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(100.0)),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Insert 50 for segment Singapore
    /*
        Italy, 100.0
        Italy, 100.0
        -------------
        MAX = 100.0

        Singapore, 50.0
        ---------------
        MAX = 50.0
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Singapore".to_string()),
                Field::Float(OrderedFloat(50.0)),
                Field::Float(OrderedFloat(50.0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::String("Singapore".to_string()),
                Field::Float(OrderedFloat(50.0)),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Update Singapore segment to Italy
    /*
        Italy, 100.0
        Italy, 100.0
        Italy, 50.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Singapore".to_string()),
                Field::Float(OrderedFloat(50.0)),
                Field::Float(OrderedFloat(50.0)),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(50.0)),
                Field::Float(OrderedFloat(50.0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![
        Operation::Update {
            old: Record::new(
                None,
                vec![
                    Field::String("Italy".to_string()),
                    Field::Float(OrderedFloat(100.0)),
                ],
            ),
            new: Record::new(
                None,
                vec![
                    Field::String("Italy".to_string()),
                    Field::Float(OrderedFloat(100.0)),
                ],
            ),
        },
        Operation::Delete {
            old: Record::new(
                None,
                vec![
                    Field::String("Singapore".to_string()),
                    Field::Float(OrderedFloat(50.0)),
                ],
            ),
        },
    ];
    assert_eq!(out, exp);

    // Update Italy value 100 -> 200
    /*
        Italy, 200.0
        Italy, 100.0
        Italy, 50.0
        -------------
        MAX = 200.0
    */
    let inp = Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(100.0)),
                Field::Float(OrderedFloat(100.0)),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(200.0)),
                Field::Float(OrderedFloat(200.0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(100.0)),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(200.0)),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Delete 1 record (200)
    /*
        Italy, 100.0
        Italy, 50.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(200.0)),
                Field::Float(OrderedFloat(200.0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(200.0)),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(100.0)),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Delete another record (50)
    /*
        Italy, 100.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(50.0)),
                Field::Float(OrderedFloat(50.0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(100.0)),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(100.0)),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Delete last record
    /*
        -------------
        MAX = Null
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(100.0)),
                Field::Float(OrderedFloat(100.0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Float(OrderedFloat(100.0)),
            ],
        ),
    }];
    assert_eq!(out, exp);
}

#[test]
fn test_max_aggregation_int() {
    let (mut processor, tx) = init_processor(
        "SELECT Country, MAX(Salary) \
        FROM Users \
        WHERE Salary >= 1 GROUP BY Country",
    )
    .unwrap();

    let schema = Schema::empty()
        .field(
            FieldDefinition::new(String::from("ID"), FieldType::Int, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("Country"), FieldType::String, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("Salary"), FieldType::Int, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("MAX(Salary)"), FieldType::Int, false),
            false,
            false,
        )
        .clone();

    let _output_schema = processor
        .update_schema(
            DEFAULT_PORT_HANDLE,
            &HashMap::from([(DEFAULT_PORT_HANDLE, schema)]),
        )
        .unwrap();

    // Insert 100 for segment Italy
    /*
        Italy, 100.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Int(100),
                Field::Int(100),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Insert {
        new: Record::new(
            None,
            vec![Field::String("Italy".to_string()), Field::Int(100)],
        ),
    }];
    assert_eq!(out, exp);

    // Insert another 100 for segment Italy
    /*
        Italy, 100.0
        Italy, 100.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Int(100),
                Field::Int(100),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![Field::String("Italy".to_string()), Field::Int(100)],
        ),
        new: Record::new(
            None,
            vec![Field::String("Italy".to_string()), Field::Int(100)],
        ),
    }];
    assert_eq!(out, exp);

    // Insert 50 for segment Singapore
    /*
        Italy, 100.0
        Italy, 100.0
        -------------
        MAX = 100.0

        Singapore, 50.0
        ---------------
        MAX = 50.0
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Singapore".to_string()),
                Field::Int(50),
                Field::Int(50),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Insert {
        new: Record::new(
            None,
            vec![Field::String("Singapore".to_string()), Field::Int(50)],
        ),
    }];
    assert_eq!(out, exp);

    // Update Singapore segment to Italy
    /*
        Italy, 100.0
        Italy, 100.0
        Italy, 50.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Singapore".to_string()),
                Field::Int(50),
                Field::Int(50),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Int(50),
                Field::Int(50),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![
        Operation::Update {
            old: Record::new(
                None,
                vec![Field::String("Italy".to_string()), Field::Int(100)],
            ),
            new: Record::new(
                None,
                vec![Field::String("Italy".to_string()), Field::Int(100)],
            ),
        },
        Operation::Delete {
            old: Record::new(
                None,
                vec![Field::String("Singapore".to_string()), Field::Int(50)],
            ),
        },
    ];
    assert_eq!(out, exp);

    // Update Italy value 100 -> 200
    /*
        Italy, 200.0
        Italy, 100.0
        Italy, 50.0
        -------------
        MAX = 200.0
    */
    let inp = Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Int(100),
                Field::Int(100),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Int(200),
                Field::Int(200),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![Field::String("Italy".to_string()), Field::Int(100)],
        ),
        new: Record::new(
            None,
            vec![Field::String("Italy".to_string()), Field::Int(200)],
        ),
    }];
    assert_eq!(out, exp);

    // Delete 1 record (200)
    /*
        Italy, 100.0
        Italy, 50.0
        -------------
        MAX = 50.0
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Int(200),
                Field::Int(200),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![Field::String("Italy".to_string()), Field::Int(200)],
        ),
        new: Record::new(
            None,
            vec![Field::String("Italy".to_string()), Field::Int(100)],
        ),
    }];
    assert_eq!(out, exp);

    // Delete another record (50)
    /*
        Italy, 100.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Int(50),
                Field::Int(50),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![Field::String("Italy".to_string()), Field::Int(100)],
        ),
        new: Record::new(
            None,
            vec![Field::String("Italy".to_string()), Field::Int(100)],
        ),
    }];
    assert_eq!(out, exp);

    // Delete last record
    /*
        -------------
        MAX = Null
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Int(100),
                Field::Int(100),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Delete {
        old: Record::new(
            None,
            vec![Field::String("Italy".to_string()), Field::Int(100)],
        ),
    }];
    assert_eq!(out, exp);
}

#[test]
fn test_max_aggregation_decimal() {
    let (mut processor, tx) = init_processor(
        "SELECT Country, MAX(Salary) \
        FROM Users \
        WHERE Salary >= 1 GROUP BY Country",
    )
    .unwrap();

    let schema = Schema::empty()
        .field(
            FieldDefinition::new(String::from("ID"), FieldType::Int, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("Country"), FieldType::String, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("Salary"), FieldType::Decimal, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("MAX(Salary)"), FieldType::Decimal, false),
            false,
            false,
        )
        .clone();

    let _output_schema = processor
        .update_schema(
            DEFAULT_PORT_HANDLE,
            &HashMap::from([(DEFAULT_PORT_HANDLE, schema)]),
        )
        .unwrap();

    // Insert 100 for segment Italy
    /*
        Italy, 100.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(100, 0)),
                Field::Decimal(Decimal::new(100, 0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(100, 0)),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Insert another 100 for segment Italy
    /*
        Italy, 100.0
        Italy, 100.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(100, 0)),
                Field::Decimal(Decimal::new(100, 0)),
            ],
        ),
    };
    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(100, 0)),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(100, 0)),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Insert 50 for segment Singapore
    /*
        Italy, 100.0
        Italy, 100.0
        -------------
        MAX = 100.0

        Singapore, 50.0
        -------------
        MAX = 50.0
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Singapore".to_string()),
                Field::Decimal(Decimal::new(50, 0)),
                Field::Decimal(Decimal::new(50, 0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::String("Singapore".to_string()),
                Field::Decimal(Decimal::new(50, 0)),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Update Singapore segment to Italy
    /*
        Italy, 100.0
        Italy, 100.0
        Italy, 50.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Singapore".to_string()),
                Field::Decimal(Decimal::new(50, 0)),
                Field::Decimal(Decimal::new(50, 0)),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(50, 0)),
                Field::Decimal(Decimal::new(50, 0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![
        Operation::Update {
            old: Record::new(
                None,
                vec![
                    Field::String("Italy".to_string()),
                    Field::Decimal(Decimal::new(100, 0)),
                ],
            ),
            new: Record::new(
                None,
                vec![
                    Field::String("Italy".to_string()),
                    Field::Decimal(Decimal::new(100, 0)),
                ],
            ),
        },
        Operation::Delete {
            old: Record::new(
                None,
                vec![
                    Field::String("Singapore".to_string()),
                    Field::Decimal(Decimal::new(50, 0)),
                ],
            ),
        },
    ];
    assert_eq!(out, exp);

    // Update Italy value 100 -> 200
    /*
        Italy, 200.0
        Italy, 100.0
        Italy, 50.0
        -------------
        MAX = 200.0
    */
    let inp = Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(100, 0)),
                Field::Decimal(Decimal::new(100, 0)),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(200, 0)),
                Field::Decimal(Decimal::new(200, 0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(100, 0)),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(200, 0)),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Delete 1 record (200)
    /*
        Italy, 100.0
        Italy, 50.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(200, 0)),
                Field::Decimal(Decimal::new(200, 0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(200, 0)),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(100, 0)),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Delete another record (50)
    /*
        Italy, 100.0
        -------------
        MAX = 100.0
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(50, 0)),
                Field::Decimal(Decimal::new(50, 0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(100, 0)),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(100, 0)),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Delete last record
    /*
        -------------
        MAX = 0.0
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(100, 0)),
                Field::Decimal(Decimal::new(100, 0)),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Decimal(Decimal::new(100, 0)),
            ],
        ),
    }];
    assert_eq!(out, exp);
}

#[test]
fn test_max_aggregation_timestamp() {
    let (mut processor, tx) = init_processor(
        "SELECT Country, MAX(StartTime) \
        FROM Users \
        WHERE StartTime <= timestamp(CURRENT_DATE()) GROUP BY Country",
    )
    .unwrap();

    let schema = Schema::empty()
        .field(
            FieldDefinition::new(String::from("ID"), FieldType::Int, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("Country"), FieldType::String, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("StartTime"), FieldType::Timestamp, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("MAX(StartTime)"), FieldType::Timestamp, false),
            false,
            false,
        )
        .clone();

    let _output_schema = processor
        .update_schema(
            DEFAULT_PORT_HANDLE,
            &HashMap::from([(DEFAULT_PORT_HANDLE, schema)]),
        )
        .unwrap();

    // Insert 100 for segment Italy
    /*
        Italy, 100
        -------------
        MAX = 100
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Insert another 100 for segment Italy
    /*
        Italy, 100
        Italy, 100
        -------------
        MAX = 100
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
            ],
        ),
    };
    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Insert 50 for segment Singapore
    /*
        Italy, 100
        Italy, 100
        -------------
        MAX = 100

        Singapore, 50
        -------------
        MAX = 50
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Singapore".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(50))),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(50))),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::String("Singapore".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(50))),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Update Singapore segment to Italy
    /*
        Italy, 100
        Italy, 100
        Italy, 50
        -------------
        MAX = 100
    */
    let inp = Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Singapore".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(50))),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(50))),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(50))),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(50))),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![
        Operation::Update {
            old: Record::new(
                None,
                vec![
                    Field::String("Italy".to_string()),
                    Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
                ],
            ),
            new: Record::new(
                None,
                vec![
                    Field::String("Italy".to_string()),
                    Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
                ],
            ),
        },
        Operation::Delete {
            old: Record::new(
                None,
                vec![
                    Field::String("Singapore".to_string()),
                    Field::Timestamp(DateTime::from(Utc.timestamp_millis(50))),
                ],
            ),
        },
    ];
    assert_eq!(out, exp);

    // Update Italy value 100 -> 200
    /*
        Italy, 200
        Italy, 100
        Italy, 50
        -------------
        MAX = 200
    */
    let inp = Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(200))),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(200))),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(200))),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Delete 1 record (200)
    /*
        Italy, 100
        Italy, 50
        -------------
        MAX = 100
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(200))),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(200))),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(200))),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Delete another record (50)
    /*
        Italy, 100
        -------------
        MAX = 100
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(50))),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(50))),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Delete last record
    /*
        -------------
        MAX = 0
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Timestamp(DateTime::from(Utc.timestamp_millis(100))),
            ],
        ),
    }];
    assert_eq!(out, exp);
}

#[test]
fn test_max_aggregation_date() {
    let (mut processor, tx) = init_processor(
        "SELECT Country, MAX(StartDate) \
        FROM Users \
        WHERE StartDate <= CURRENT_DATE() GROUP BY Country",
    )
    .unwrap();

    let schema = Schema::empty()
        .field(
            FieldDefinition::new(String::from("ID"), FieldType::Int, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("Country"), FieldType::String, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("StartDate"), FieldType::Date, false),
            false,
            false,
        )
        .field(
            FieldDefinition::new(String::from("MAX(StartDate)"), FieldType::Date, false),
            false,
            false,
        )
        .clone();

    let _output_schema = processor
        .update_schema(
            DEFAULT_PORT_HANDLE,
            &HashMap::from([(DEFAULT_PORT_HANDLE, schema)]),
        )
        .unwrap();

    // Insert 2015-10-08 for segment Italy
    /*
        Italy, 2015-10-08
        ------------------
        MAX = 2015-10-08
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Insert another 2015-10-08 for segment Italy
    /*
        Italy, 2015-10-08
        Italy, 2015-10-08
        -----------------
        MAX = 2015-10-08
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
            ],
        ),
    };
    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Insert 2015-10-04 for segment Singapore
    /*
        Italy, 2015-10-08
        Italy, 2015-10-08
        -------------
        MAX = 2015-10-08

        Singapore, 2015-10-04
        -------------
        MAX = 2015-10-04
    */
    let inp = Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Singapore".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-04", DATE_FORMAT).unwrap()),
                Field::Date(NaiveDate::parse_from_str("2015-10-04", DATE_FORMAT).unwrap()),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Insert {
        new: Record::new(
            None,
            vec![
                Field::String("Singapore".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-04", DATE_FORMAT).unwrap()),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Update Singapore segment to Italy
    /*
        Italy, 2015-10-08
        Italy, 2015-10-08
        Italy, 2015-10-04
        -------------
        MAX = 2015-10-08
    */
    let inp = Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Singapore".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-04", DATE_FORMAT).unwrap()),
                Field::Date(NaiveDate::parse_from_str("2015-10-04", DATE_FORMAT).unwrap()),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-04", DATE_FORMAT).unwrap()),
                Field::Date(NaiveDate::parse_from_str("2015-10-04", DATE_FORMAT).unwrap()),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![
        Operation::Update {
            old: Record::new(
                None,
                vec![
                    Field::String("Italy".to_string()),
                    Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
                ],
            ),
            new: Record::new(
                None,
                vec![
                    Field::String("Italy".to_string()),
                    Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
                ],
            ),
        },
        Operation::Delete {
            old: Record::new(
                None,
                vec![
                    Field::String("Singapore".to_string()),
                    Field::Date(NaiveDate::parse_from_str("2015-10-04", DATE_FORMAT).unwrap()),
                ],
            ),
        },
    ];
    assert_eq!(out, exp);

    // Update Italy value 100 -> 200
    /*
        Italy, 2015-10-16
        Italy, 2015-10-08
        Italy, 2015-10-04
        -------------
        MAX = 2015-10-16
    */
    let inp = Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-16", DATE_FORMAT).unwrap()),
                Field::Date(NaiveDate::parse_from_str("2015-10-16", DATE_FORMAT).unwrap()),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-16", DATE_FORMAT).unwrap()),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Delete 1 record (2015-10-16)
    /*
        Italy, 2015-10-08
        Italy, 2015-10-04
        -------------
        MAX = 2015-10-08
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-16", DATE_FORMAT).unwrap()),
                Field::Date(NaiveDate::parse_from_str("2015-10-16", DATE_FORMAT).unwrap()),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-16", DATE_FORMAT).unwrap()),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Delete another record (2015-10-04)
    /*
        Italy, 2015-10-08
        -------------
        MAX = 2015-10-08
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-04", DATE_FORMAT).unwrap()),
                Field::Date(NaiveDate::parse_from_str("2015-10-04", DATE_FORMAT).unwrap()),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Update {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
            ],
        ),
        new: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
            ],
        ),
    }];
    assert_eq!(out, exp);

    // Delete last record
    /*
        -------------
        MAX = 0
    */
    let inp = Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::Int(0),
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
            ],
        ),
    };

    let out = processor
        .aggregate(
            &mut SharedTransaction::new(&tx),
            &processor.db.clone().unwrap(),
            inp,
        )
        .unwrap_or_else(|_e| panic!("Error executing aggregate"));

    let exp = vec![Operation::Delete {
        old: Record::new(
            None,
            vec![
                Field::String("Italy".to_string()),
                Field::Date(NaiveDate::parse_from_str("2015-10-08", DATE_FORMAT).unwrap()),
            ],
        ),
    }];
    assert_eq!(out, exp);
}