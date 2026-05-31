//! Relational algebra operations: select, project, join, union, difference, rename.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single value in a relation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Int(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    Null,
}

impl Value {
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a.partial_cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b),
            (Value::Text(a), Value::Text(b)) => a.partial_cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

/// A tuple (row) in a relation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tuple {
    pub values: Vec<Value>,
}

impl Tuple {
    pub fn new(values: Vec<Value>) -> Self {
        Tuple { values }
    }

    pub fn get(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }
}

/// A relation (table) with named attributes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub schema: Vec<String>,
    pub tuples: Vec<Tuple>,
}

impl Relation {
    pub fn new(schema: Vec<String>) -> Self {
        Relation {
            schema,
            tuples: Vec::new(),
        }
    }

    pub fn with_tuples(schema: Vec<String>, tuples: Vec<Tuple>) -> Self {
        // Validate arity
        for t in &tuples {
            assert_eq!(t.values.len(), schema.len(), "Tuple arity mismatch");
        }
        Relation { schema, tuples }
    }

    pub fn arity(&self) -> usize {
        self.schema.len()
    }

    pub fn cardinality(&self) -> usize {
        self.tuples.len()
    }

    /// Returns the index of a column by name.
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.schema.iter().position(|s| s == name)
    }

    /// σ (select): filter rows by predicate on column name.
    pub fn select<F>(&self, predicate: F) -> Relation
    where
        F: Fn(&Tuple, &[String]) -> bool,
    {
        let filtered: Vec<Tuple> = self
            .tuples
            .iter()
            .filter(|t| predicate(t, &self.schema))
            .cloned()
            .collect();
        Relation {
            schema: self.schema.clone(),
            tuples: filtered,
        }
    }

    /// π (project): keep only specified columns.
    pub fn project(&self, columns: &[&str]) -> Relation {
        let indices: Vec<usize> = columns
            .iter()
            .map(|c| {
                self.column_index(c)
                    .unwrap_or_else(|| panic!("Column '{}' not found", c))
            })
            .collect();

        let new_schema: Vec<String> = indices.iter().map(|&i| self.schema[i].clone()).collect();
        let new_tuples: Vec<Tuple> = self
            .tuples
            .iter()
            .map(|t| {
                let vals: Vec<Value> = indices.iter().map(|&i| t.values[i].clone()).collect();
                Tuple::new(vals)
            })
            .collect();

        // Remove duplicates (projection is a set operation)
        let mut seen = Vec::new();
        for t in new_tuples {
            if !seen.contains(&t) {
                seen.push(t);
            }
        }

        Relation {
            schema: new_schema,
            tuples: seen,
        }
    }

    /// ⋈ (natural join): join on columns with matching names.
    pub fn join(&self, other: &Relation) -> Relation {
        // Find common columns
        let common: Vec<(usize, usize)> = self
            .schema
            .iter()
            .enumerate()
            .filter_map(|(i, name)| {
                other
                    .column_index(name)
                    .map(|j| (i, j))
            })
            .collect();

        if common.is_empty() {
            // Cross product
            return self.cross_product(other);
        }

        // Build output schema: all from self + non-common from other
        let common_names: Vec<String> = common.iter().map(|(i, _)| self.schema[*i].clone()).collect();
        let other_extras: Vec<(usize, String)> = other
            .schema
            .iter()
            .enumerate()
            .filter(|(_j, name)| !common_names.contains(name))
            .map(|(j, name)| (j, name.clone()))
            .collect();

        let mut out_schema = self.schema.clone();
        for (_, name) in &other_extras {
            out_schema.push(name.clone());
        }

        let mut out_tuples = Vec::new();
        for t1 in &self.tuples {
            for t2 in &other.tuples {
                let matches = common
                    .iter()
                    .all(|(i, j)| t1.values[*i] == t2.values[*j]);
                if matches {
                    let mut vals = t1.values.clone();
                    for (j, _) in &other_extras {
                        vals.push(t2.values[*j].clone());
                    }
                    out_tuples.push(Tuple::new(vals));
                }
            }
        }

        Relation {
            schema: out_schema,
            tuples: out_tuples,
        }
    }

    /// θ-join (theta join): cross product filtered by predicate.
    pub fn theta_join<F>(&self, other: &Relation, predicate: F) -> Relation
    where
        F: Fn(&Tuple, &Tuple) -> bool,
    {
        let combined_schema: Vec<String> = self
            .schema
            .iter()
            .chain(other.schema.iter())
            .cloned()
            .collect();

        let mut tuples = Vec::new();
        for t1 in &self.tuples {
            for t2 in &other.tuples {
                if predicate(t1, t2) {
                    let vals: Vec<Value> = t1
                        .values
                        .iter()
                        .chain(t2.values.iter())
                        .cloned()
                        .collect();
                    tuples.push(Tuple::new(vals));
                }
            }
        }

        Relation {
            schema: combined_schema,
            tuples,
        }
    }

    /// Cross product.
    pub fn cross_product(&self, other: &Relation) -> Relation {
        let combined_schema: Vec<String> = self
            .schema
            .iter()
            .chain(other.schema.iter())
            .cloned()
            .collect();

        let mut tuples = Vec::new();
        for t1 in &self.tuples {
            for t2 in &other.tuples {
                let vals: Vec<Value> = t1
                    .values
                    .iter()
                    .chain(t2.values.iter())
                    .cloned()
                    .collect();
                tuples.push(Tuple::new(vals));
            }
        }

        Relation {
            schema: combined_schema,
            tuples,
        }
    }

    /// ∪ (union): set union of two union-compatible relations.
    pub fn union(&self, other: &Relation) -> Relation {
        assert_eq!(self.schema, other.schema, "Schema mismatch for union");
        let mut result = self.clone();
        for t in &other.tuples {
            if !result.tuples.contains(t) {
                result.tuples.push(t.clone());
            }
        }
        result
    }

    /// − (difference): set difference self \ other.
    pub fn difference(&self, other: &Relation) -> Relation {
        assert_eq!(self.schema, other.schema, "Schema mismatch for difference");
        let tuples: Vec<Tuple> = self
            .tuples
            .iter()
            .filter(|t| !other.tuples.contains(t))
            .cloned()
            .collect();
        Relation {
            schema: self.schema.clone(),
            tuples,
        }
    }

    /// ρ (rename): rename columns.
    pub fn rename(&self, renames: &HashMap<String, String>) -> Relation {
        let schema: Vec<String> = self
            .schema
            .iter()
            .map(|name| renames.get(name).cloned().unwrap_or_else(|| name.clone()))
            .collect();
        Relation {
            schema,
            tuples: self.tuples.clone(),
        }
    }

    /// Intersection: self ∩ other.
    pub fn intersection(&self, other: &Relation) -> Relation {
        assert_eq!(self.schema, other.schema, "Schema mismatch for intersection");
        let tuples: Vec<Tuple> = self
            .tuples
            .iter()
            .filter(|t| other.tuples.contains(t))
            .cloned()
            .collect();
        Relation {
            schema: self.schema.clone(),
            tuples,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_emp() -> Relation {
        Relation::with_tuples(
            vec!["id".into(), "name".into(), "dept".into()],
            vec![
                Tuple::new(vec![Value::Int(1), Value::Text("Alice".into()), Value::Text("Eng".into())]),
                Tuple::new(vec![Value::Int(2), Value::Text("Bob".into()), Value::Text("Sales".into())]),
                Tuple::new(vec![Value::Int(3), Value::Text("Carol".into()), Value::Text("Eng".into())]),
            ],
        )
    }

    fn make_dept() -> Relation {
        Relation::with_tuples(
            vec!["dept".into(), "budget".into()],
            vec![
                Tuple::new(vec![Value::Text("Eng".into()), Value::Int(500)]),
                Tuple::new(vec![Value::Text("Sales".into()), Value::Int(300)]),
            ],
        )
    }

    #[test]
    fn test_select() {
        let emp = make_emp();
        let eng = emp.select(|t, schema| {
            let idx = schema.iter().position(|s| s == "dept").unwrap();
            t.values[idx] == Value::Text("Eng".into())
        });
        assert_eq!(eng.cardinality(), 2);
        assert_eq!(eng.tuples[0].get(1).unwrap().as_str(), Some("Alice"));
        assert_eq!(eng.tuples[1].get(1).unwrap().as_str(), Some("Carol"));
    }

    #[test]
    fn test_project() {
        let emp = make_emp();
        let names = emp.project(&["name"]);
        assert_eq!(names.cardinality(), 3);
        assert_eq!(names.schema, vec!["name"]);
    }

    #[test]
    fn test_project_dedup() {
        let emp = make_emp();
        let depts = emp.project(&["dept"]);
        assert_eq!(depts.cardinality(), 2); // Eng, Sales
    }

    #[test]
    fn test_natural_join() {
        let emp = make_emp();
        let dept = make_dept();
        let joined = emp.join(&dept);
        // Schema: id, name, dept, budget
        assert_eq!(joined.arity(), 4);
        assert_eq!(joined.cardinality(), 3);
        // Check that Alice (Eng) got budget 500
        let alice = &joined.tuples[0];
        assert_eq!(alice.get(3).unwrap(), &Value::Int(500));
    }

    #[test]
    fn test_theta_join() {
        let emp = make_emp();
        let dept = make_dept();
        // Join where emp.dept = dept.dept AND budget > 400
        let result = emp.theta_join(&dept, |t1, t2| {
            t1.values[2] == t2.values[0] && t2.values[1] == Value::Int(500)
        });
        assert_eq!(result.cardinality(), 2); // Alice and Carol in Eng with budget 500
    }

    #[test]
    fn test_union() {
        let r1 = Relation::with_tuples(
            vec!["a".into()],
            vec![Tuple::new(vec![Value::Int(1)]), Tuple::new(vec![Value::Int(2)])],
        );
        let r2 = Relation::with_tuples(
            vec!["a".into()],
            vec![Tuple::new(vec![Value::Int(2)]), Tuple::new(vec![Value::Int(3)])],
        );
        let u = r1.union(&r2);
        assert_eq!(u.cardinality(), 3);
    }

    #[test]
    fn test_difference() {
        let r1 = Relation::with_tuples(
            vec!["a".into()],
            vec![Tuple::new(vec![Value::Int(1)]), Tuple::new(vec![Value::Int(2)])],
        );
        let r2 = Relation::with_tuples(
            vec!["a".into()],
            vec![Tuple::new(vec![Value::Int(2)]), Tuple::new(vec![Value::Int(3)])],
        );
        let d = r1.difference(&r2);
        assert_eq!(d.cardinality(), 1);
        assert_eq!(d.tuples[0].get(0).unwrap(), &Value::Int(1));
    }

    #[test]
    fn test_rename() {
        let emp = make_emp();
        let mut renames = HashMap::new();
        renames.insert("name".into(), "employee_name".into());
        let renamed = emp.rename(&renames);
        assert_eq!(renamed.schema, vec!["id", "employee_name", "dept"]);
        assert_eq!(renamed.cardinality(), 3);
    }

    #[test]
    fn test_intersection() {
        let r1 = Relation::with_tuples(
            vec!["a".into()],
            vec![Tuple::new(vec![Value::Int(1)]), Tuple::new(vec![Value::Int(2)])],
        );
        let r2 = Relation::with_tuples(
            vec!["a".into()],
            vec![Tuple::new(vec![Value::Int(2)]), Tuple::new(vec![Value::Int(3)])],
        );
        let i = r1.intersection(&r2);
        assert_eq!(i.cardinality(), 1);
        assert_eq!(i.tuples[0].get(0).unwrap(), &Value::Int(2));
    }

    #[test]
    fn test_cross_product() {
        let r1 = Relation::with_tuples(vec!["x".into()], vec![Tuple::new(vec![Value::Int(1)])]);
        let r2 = Relation::with_tuples(
            vec!["y".into()],
            vec![Tuple::new(vec![Value::Int(10)]), Tuple::new(vec![Value::Int(20)])],
        );
        let cp = r1.cross_product(&r2);
        assert_eq!(cp.cardinality(), 2);
        assert_eq!(cp.arity(), 2);
    }
}
