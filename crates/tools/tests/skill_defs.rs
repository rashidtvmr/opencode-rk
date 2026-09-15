use opencode_rk_tools::skill_defs::{MAX_SKILL_DEFS, SkillDefError, qualify_skills};

#[test]
fn skd_t01_valid() {
    let out = qualify_skills(&[("review", "review code"), ("plan", "plan work")]).unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].name, "review");
    assert_eq!(out[0].desc, "review code");
    assert_eq!(out[1].name, "plan");
    assert_eq!(out[1].desc, "plan work");
}

#[test]
fn skd_t02_empty_name() {
    assert_eq!(
        qualify_skills(&[("", "d")]).unwrap_err(),
        SkillDefError::EmptyName
    );
}

#[test]
fn skd_t03_empty_desc() {
    assert_eq!(
        qualify_skills(&[("review", "")]).unwrap_err(),
        SkillDefError::EmptyDesc
    );
}

#[test]
fn skd_t04_overflow() {
    let names: Vec<String> = (0..MAX_SKILL_DEFS + 1).map(|i| format!("s{i}")).collect();
    let items: Vec<(&str, &str)> = names.iter().map(|s| (s.as_str(), "d")).collect();
    assert_eq!(
        qualify_skills(&items).unwrap_err(),
        SkillDefError::TooMany {
            max: MAX_SKILL_DEFS,
            actual: MAX_SKILL_DEFS + 1
        }
    );
}

#[test]
fn skd_t05_order() {
    let out = qualify_skills(&[("b", "1"), ("a", "2"), ("c", "3")]).unwrap();
    let names: Vec<&str> = out.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["b", "a", "c"]);
}
