//! The `.deploy` semantic walker: applications, components, profiles,
//! deployments, mappings, and setting paths.

use phero_syntax::ast::{
    DeployApplicationDecl, DeployComponentDecl, DeployDeploymentDecl, DeployFile,
    DeployProfileDecl, DeploySettingDecl, DeploySettingValue,
};

use crate::class::HighlightClass;
use crate::engine::semantic::Ctx;

/// Walks a parsed `.deploy` file.
pub(crate) fn walk(ctx: &mut Ctx, file: &DeployFile) {
    for application in &file.applications {
        walk_application(ctx, application);
    }
    for profile in &file.profiles {
        walk_profile(ctx, profile);
    }
    for deployment in &file.deployments {
        walk_deployment(ctx, deployment);
    }
}

fn walk_application(ctx: &mut Ctx, application: &DeployApplicationDecl) {
    ctx.decl_name(&application.name, HighlightClass::Namespace);
    for component in &application.components {
        walk_component(ctx, component);
    }
    for connection in &application.connections {
        ctx.push(connection.from.span, HighlightClass::Type);
        ctx.push(connection.to.span, HighlightClass::Type);
    }
}

fn walk_component(ctx: &mut Ctx, component: &DeployComponentDecl) {
    ctx.decl_name(&component.name, HighlightClass::Type);
    ctx.push(component.kind.span, HighlightClass::Constant);
    for capability in &component.requires {
        ctx.push(capability.span, HighlightClass::Function);
    }
    for setting in &component.settings {
        walk_setting(ctx, setting);
    }
    for env in &component.env {
        ctx.push(env.name.span, HighlightClass::Constant);
    }
}

fn walk_profile(ctx: &mut Ctx, profile: &DeployProfileDecl) {
    ctx.decl_name(&profile.name, HighlightClass::Namespace);
    if let Some(target) = &profile.target {
        ctx.push(target.span, HighlightClass::Constant);
    }
    for setting in &profile.defaults {
        walk_setting(ctx, setting);
    }
    for policy in &profile.policies {
        ctx.expression(policy.expr);
    }
    for mapping in &profile.mappings {
        ctx.push(mapping.kind.span, HighlightClass::Constant);
        ctx.qualified(&mapping.resource, HighlightClass::Namespace);
    }
}

fn walk_deployment(ctx: &mut Ctx, deployment: &DeployDeploymentDecl) {
    ctx.decl_name(&deployment.name, HighlightClass::Function);
    ctx.push(deployment.application.span, HighlightClass::Type);
    if let Some(profile) = &deployment.profile {
        ctx.push(profile.span, HighlightClass::Namespace);
    }
    for setting in &deployment.configure {
        walk_setting(ctx, setting);
    }
}

fn walk_setting(ctx: &mut Ctx, setting: &DeploySettingDecl) {
    ctx.member_path(&setting.path, HighlightClass::Property);
    if let DeploySettingValue::Word(word) = &setting.value {
        ctx.push(word.span, HighlightClass::Constant);
    }
}
