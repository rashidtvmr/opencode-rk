use opencode_rk_tools::skill_commands::{
    CommandEntry, MAX_COMMANDS, MAX_SKILLS, SkillEntry, SkillRegistry,
};

fn skill(name: &str) -> SkillEntry {
    SkillEntry {
        name: name.to_string(),
        description: format!("desc for {name}"),
    }
}

fn command(name: &str) -> CommandEntry {
    CommandEntry {
        name: name.to_string(),
        description: format!("desc for {name}"),
    }
}

#[test]
fn skills_t01_register_and_lookup() {
    let mut reg = SkillRegistry::new();
    reg.register_skill(skill("alpha")).unwrap();
    reg.register_command(command("/alpha")).unwrap();
    assert_eq!(reg.skill("alpha").unwrap().description, "desc for alpha");
    assert_eq!(reg.command("/alpha").unwrap().description, "desc for /alpha");
    assert_eq!(reg.skills().len(), 1);
    assert_eq!(reg.commands().len(), 1);
}

#[test]
fn skills_t02_empty_name_rejected() {
    let mut reg = SkillRegistry::new();
    assert!(matches!(
        reg.register_skill(skill("")),
        Err(opencode_rk_tools::skill_commands::SkillRegistryError::EmptyName)
    ));
    assert!(matches!(
        reg.register_command(command("")),
        Err(opencode_rk_tools::skill_commands::SkillRegistryError::EmptyName)
    ));
}

#[test]
fn skills_t03_duplicate_rejected() {
    let mut reg = SkillRegistry::new();
    reg.register_skill(skill("dup")).unwrap();
    let err = reg.register_skill(skill("dup")).unwrap_err();
    assert!(matches!(
        err,
        opencode_rk_tools::skill_commands::SkillRegistryError::DuplicateName { ref name }
        if name == "dup"
    ));
    reg.register_command(command("/dup")).unwrap();
    let err = reg.register_command(command("/dup")).unwrap_err();
    assert!(matches!(
        err,
        opencode_rk_tools::skill_commands::SkillRegistryError::DuplicateName { ref name }
        if name == "/dup"
    ));
}

#[test]
fn skills_t04_commands_separate_namespace() {
    let mut reg = SkillRegistry::new();
    reg.register_skill(skill("shared")).unwrap();
    // Same name in command namespace must be allowed.
    reg.register_command(command("shared")).unwrap();
    assert!(reg.skill("shared").is_some());
    assert!(reg.command("shared").is_some());
}

#[test]
fn skills_t05_overflow_rejected() {
    let mut reg = SkillRegistry::new();
    for i in 0..MAX_SKILLS {
        reg.register_skill(skill(&format!("s{i:03}"))).unwrap();
    }
    let err = reg.register_skill(skill("one-too-many")).unwrap_err();
    assert!(matches!(
        err,
        opencode_rk_tools::skill_commands::SkillRegistryError::TooManySkills { max, actual }
        if max == MAX_SKILLS && actual == MAX_SKILLS + 1
    ));

    let mut reg = SkillRegistry::new();
    for i in 0..MAX_COMMANDS {
        reg.register_command(command(&format!("/c{i:03}"))).unwrap();
    }
    let err = reg.register_command(command("/overflow")).unwrap_err();
    assert!(matches!(
        err,
        opencode_rk_tools::skill_commands::SkillRegistryError::TooManyCommands { max, actual }
        if max == MAX_COMMANDS && actual == MAX_COMMANDS + 1
    ));
}
