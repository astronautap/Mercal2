// src/web/admin_handlers.rs
use crate::{
    error::{AppError, AppResult},
    // models::user::User, // Removido (não usado diretamente aqui)
    services::user_service, // Funções de gestão de users
    state::AppState,
    // Structs Askama e wrapper UserWithRoles
    templates::{AdminEditUserPage, AdminUsersPage, UserWithRoles},
    // web::mw_auth::UserId, // Removido (não usado diretamente aqui)
};
// Adicionar imports necessários
use askama::Template; // Para render()
use axum::{
    extract::{Form, Path, Query, State}, // Adicionar Query para feedback
    response::{Html, IntoResponse, Redirect}, // Adicionar Html
};
use serde::Deserialize;
use std::collections::HashMap; // Para processar form
// Adicionar import urlencoding
use urlencoding;

// --- Structs para os Formulários ---
#[derive(Deserialize, Debug)]
pub struct CreateUserForm {
    id: String,
    name: String,
    password: String,
    turma: String,
    ano: i64,
    curso: String,
    genero: String,
    // Espera diretamente um vetor para múltiplos valores com o mesmo nome 'roles'
    #[serde(default)] // Usa vetor vazio se nenhum 'roles' for enviado
    roles: Vec<String>,
    // Remover 'extra' se não houver outros campos flatten
    // #[serde(flatten)]
    // extra: HashMap<String, String>,
}

// EditUserForm (já estava correta)
#[derive(Deserialize, Debug)]
pub struct EditUserForm {
    name: String,
    turma: String,
    ano: i64,
    curso: String,
    genero: String,
    #[serde(default)]
    roles: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct ChangePasswordForm {
    id: String,
    new_password: String,
}

#[derive(Deserialize, Debug)]
pub struct FeedbackParams {
    success: Option<String>,
    error: Option<String>,
}

// --- Handlers ---

/// Handler para GET /admin/users - Mostra a página de gestão
pub async fn show_admin_users_page(
    State(state): State<AppState>, // Acesso ao pool da DB
    Query(params): Query<FeedbackParams>, // Recebe feedback via query params
) -> AppResult<impl IntoResponse> { // Manter impl IntoResponse
    tracing::debug!("GET /admin/users: Carregando página de gestão...");

    // 1. Busca todos os utilizadores da base de dados
    let users_result = user_service::find_all_users(&state.db_pool).await;
    let users = match users_result {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("Erro ao buscar todos os utilizadores: {:?}", e);
            // Renderiza mesmo com erro na busca
            let template = AdminUsersPage {
                users: vec![], // Lista vazia
                success_message: None,
                error_message: Some("Falha ao carregar lista de utilizadores.".to_string()),
            };
            // Tenta renderizar, retorna erro interno se falhar
            return match template.render() {
                 Ok(html) => Ok(Html(html).into_response()),
                 Err(render_e) => {
                    tracing::error!("Falha ao renderizar template AdminUsersPage (com erro de busca): {}", render_e);
                    Err(AppError::InternalServerError)
                 }
            };
        }
    };

    // 2. Para cada utilizador, busca as suas roles
    let mut users_with_roles = Vec::new();
    for user in users {
        let roles = match user_service::get_user_roles(&state.db_pool, &user.id).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    "Erro ao buscar roles para user {}: {:?}. Mostrando sem roles.",
                    user.id,
                    e
                );
                vec![] // Mostra lista vazia de roles se houver erro
            }
        };
        // Cria a struct combinada para o template
        users_with_roles.push(UserWithRoles {
            id: user.id,
            name: user.name,
            turma: user.turma,
            ano: user.ano,
            curso: user.curso,
            genero: user.genero,
            roles, // Adiciona o Vec<String> de roles
        });
    }

    // 3. Cria a struct do template Askama, passando a lista e feedback
    let template = AdminUsersPage {
        users: users_with_roles,
        success_message: params.success, // Vem da query string (?success=...)
        error_message: params.error,     // Vem da query string (?error=...)
    };

    // 4. Renderiza o template explicitamente e trata erro
    match template.render() {
        Ok(html) => Ok(Html(html).into_response()), // Retorna Ok(Html(...))
        Err(e) => {
            tracing::error!("Falha ao renderizar template AdminUsersPage: {}", e);
            Err(AppError::InternalServerError) // Retorna Err se render falhar
        }
    }
}

/// Handler para POST /admin/users/create - Cria um novo utilizador

pub async fn handle_create_user(
    State(state): State<AppState>,
    Form(form): Form<CreateUserForm>, // Usa struct corrigida
) -> AppResult<Redirect> {

    tracing::info!("POST /admin/users/create: Tentando criar user {}", form.id);

    // Validações básicas (pode adicionar mais)
    if form.id.trim().is_empty()
        || form.name.trim().is_empty()
        || form.password.len() < 4 // Exemplo: Mínimo 4 caracteres
        || form.turma.trim().is_empty()
        || form.curso.trim().is_empty()
        || (form.genero != "M" && form.genero != "F") // Garante M ou F
    {
        tracing::warn!("Criação falhou: Dados inválidos no formulário.");
        let error_msg = urlencoding::encode("Dados inválidos. Verifique todos os campos (senha mín. 4 caracteres).");
        // Criar URL numa variável antes
        let redirect_url = format!("/admin/users?error={}", error_msg);
        // Retorna Ok(Redirect) mesmo em caso de erro de validação (padrão Post/Redirect/Get)
        return Ok(Redirect::to(&redirect_url));
    }

    // Usa form.roles diretamente (já é Vec<String>)
    let roles = &form.roles;
    tracing::debug!("Roles selecionadas para {}: {:?}", form.id, roles);


    // Chama o serviço para criar o utilizador na DB
    match user_service::create_user(
        &state.db_pool,
        &form.id,
        &form.name,
        &form.password, // Passa a senha "raw"
        &form.turma,
        form.ano,
        &form.curso,
        &form.genero,
        roles, // Passa &Vec<String> (converte para &[String])
    )
    .await
    {
        Ok(_) => {
            // Sucesso! Redireciona com mensagem de sucesso
            tracing::info!("Utilizador {} criado com sucesso.", form.id);
            let success_msg = urlencoding::encode(&format!("Utilizador '{}' criado com sucesso.", form.id)).to_string();
            // Criar URL numa variável antes
            let redirect_url = format!("/admin/users?success={}", success_msg);
            Ok(Redirect::to(&redirect_url)) // Passa a referência da variável
        }
        Err(e) => {
            // Erro ao criar (ex: ID já existe, erro DB)
            tracing::error!("Erro ao criar utilizador {}: {:?}", form.id, e);
            // Tenta dar uma mensagem mais específica
            let error_detail = match e {
                // TODO: Fazer user_service retornar erro específico para ID duplicado
                _ => "ID de utilizador já existe ou ocorreu um erro na base de dados.".to_string(),
            };
            let error_msg = urlencoding::encode(&error_detail);
            // Criar URL numa variável antes
            let redirect_url = format!("/admin/users?error={}", error_msg);
            // Retorna Ok(Redirect) mesmo em caso de erro na DB (padrão PRG)
            Ok(Redirect::to(&redirect_url))
        }
    }
}

/// Handler para POST /admin/users/change_password - Altera a senha de um utilizador
pub async fn handle_change_password(
    State(state): State<AppState>, // Acesso ao pool da DB
    Form(form): Form<ChangePasswordForm>, // Dados do formulário
) -> AppResult<Redirect> { // Retorna AppResult<Redirect>

    tracing::info!("POST /admin/users/change_password: Tentando alterar senha para {}", form.id);

    // Validações básicas
    if form.id.trim().is_empty() || form.new_password.len() < 4 {
        tracing::warn!("Alteração de senha falhou: Dados inválidos.");
        let error_msg = urlencoding::encode("ID ou nova senha inválidos.");
        let redirect_url = format!("/admin/users?error={}", error_msg);
        return Ok(Redirect::to(&redirect_url));
    }

    // Chama o serviço para alterar a senha na DB
    match user_service::update_user_password(&state.db_pool, &form.id, &form.new_password).await {
        Ok(_) => {
            // Sucesso!
            tracing::info!("Senha alterada com sucesso para {}", form.id);
            let success_msg = urlencoding::encode(&format!("Senha para '{}' alterada com sucesso.", form.id)).to_string();
            let redirect_url = format!("/admin/users?success={}", success_msg);
            Ok(Redirect::to(&redirect_url))
        }
        Err(e) => {
            // Erro (ex: user não encontrado, erro DB)
            tracing::error!("Erro ao alterar senha para {}: {:?}", form.id, e);
            // Tenta dar uma mensagem mais específica
             let error_detail = match e {
                 // TODO: Fazer user_service retornar erro específico para UserNotFound
                 _ => "Utilizador não encontrado ou erro na base de dados.".to_string(),
            };
            let error_msg = urlencoding::encode(&error_detail);
            let redirect_url = format!("/admin/users?error={}", error_msg);
            Ok(Redirect::to(&redirect_url))
        }
    }
}

pub async fn show_edit_user_form(
    State(state): State<AppState>, // Acesso ao pool da DB
    Path(user_id): Path<String>, // <<< Extrai o ID da URL (ex: /admin/users/edit/1001)
) -> AppResult<impl IntoResponse> {
    tracing::debug!("GET /admin/users/edit/{} : Mostrando formulário", user_id);

    // 1. Busca os dados atuais do utilizador
    let user_result = user_service::find_user_by_id(&state.db_pool, &user_id).await;

    // Trata caso de utilizador não encontrado ou erro na DB
    let user = match user_result {
        Ok(Some(u)) => u,
        Ok(None) => {
            tracing::warn!("Tentativa de editar utilizador inexistente: {}", user_id);
            // Renderiza o template com mensagem de erro (ou retorna NotFound)
            let template = AdminEditUserPage {
                user: None, // Passa None para indicar erro
                current_user_roles: &[],
                all_defined_roles: &user_service::DEFINED_ROLES,
                error_message: Some(format!("Utilizador '{}' não encontrado.", user_id)),
            };
            return match template.render() {
                Ok(html) => Ok(Html(html).into_response()),
                Err(_) => Err(AppError::InternalServerError), // Erro ao renderizar erro
            };
        }
        Err(e) => {
            tracing::error!("Erro ao buscar user {} para edição: {:?}", user_id, e);
            // Renderiza o template com mensagem de erro genérica
             let template = AdminEditUserPage {
                user: None,
                current_user_roles: &[],
                all_defined_roles: &user_service::DEFINED_ROLES,
                error_message: Some("Erro ao carregar dados do utilizador.".to_string()),
            };
             return match template.render() {
                 Ok(html) => Ok(Html(html).into_response()),
                 Err(_) => Err(AppError::InternalServerError),
             };
        }
    };

    // 2. Busca as roles atuais do utilizador
    let current_roles = match user_service::get_user_roles(&state.db_pool, &user_id).await {
        Ok(roles) => roles,
        Err(e) => {
            tracing::error!("Erro ao buscar roles de {} para edição: {:?}", user_id, e);
            // Continua, mas mostra erro no template? Ou retorna erro 500?
            // Vamos continuar e mostrar mensagem no template.
            let template = AdminEditUserPage {
                user: Some(&user), // Passa o user encontrado
                current_user_roles: &[], // Lista vazia
                all_defined_roles: &user_service::DEFINED_ROLES,
                error_message: Some("Erro ao carregar roles atuais do utilizador.".to_string()),
            };
             return match template.render() {
                 Ok(html) => Ok(Html(html).into_response()),
                 Err(_) => Err(AppError::InternalServerError),
             };
        }
    };

    // 3. Prepara os dados e renderiza o template de edição
    let template = AdminEditUserPage {
        user: Some(&user), // Passa referência ao user encontrado
        current_user_roles: &current_roles, // Passa slice das roles atuais
        all_defined_roles: &user_service::DEFINED_ROLES, // Passa slice da constante
        error_message: None, // Sem erro nesta fase
    };

    match template.render() {
        Ok(html) => Ok(Html(html).into_response()),
        Err(e) => {
            tracing::error!("Falha ao renderizar template AdminEditUserPage para {}: {}", user_id, e);
            Err(AppError::InternalServerError)
        }
    }
}


// <<< ADICIONADO: Handler para POST /admin/users/edit/:id - Processa a edição >>>
pub async fn handle_edit_user(
    State(state): State<AppState>, // Acesso ao pool da DB
    Path(user_id): Path<String>, // ID do utilizador vindo da URL
    Form(form): Form<EditUserForm>, // Dados do formulário
) -> AppResult<Redirect> { // Redireciona para /admin/users com feedback

    tracing::info!("POST /admin/users/edit/{}: Processando edição...", user_id);

    // Validações básicas (pode adicionar mais)
     if form.name.trim().is_empty()
        || form.turma.trim().is_empty()
        || form.curso.trim().is_empty()
        || (form.genero != "M" && form.genero != "F")
    {
        tracing::warn!("Edição falhou para {}: Dados inválidos no formulário.", user_id);
        let error_msg = urlencoding::encode("Dados inválidos. Verifique todos os campos.");
        // Redireciona DE VOLTA para a página de edição com erro
        // (Alternativa: redirecionar para /admin/users com erro genérico)
        let redirect_url = format!("/admin/users/edit/{}?error={}", user_id, error_msg);
        return Ok(Redirect::to(&redirect_url));
    }

    // Chama o serviço para atualizar os dados básicos do utilizador
    let update_user_result = user_service::update_user(
        &state.db_pool, &user_id, &form.name, &form.turma,
        form.ano, &form.curso, &form.genero
    ).await;

    if let Err(e) = update_user_result {
        tracing::error!("Erro ao atualizar dados do user {}: {:?}", user_id, e);
        // Tenta dar uma mensagem mais específica
        let error_detail = match e {
             // Assumindo InternalServerError para UserNotFound
             AppError::InternalServerError => "Utilizador não encontrado.".to_string(),
            _ => "Erro ao atualizar dados na base de dados.".to_string(),
        };
        let error_msg = urlencoding::encode(&error_detail);
        // Redireciona de volta para a PÁGINA DE EDIÇÃO com erro
        let redirect_url = format!("/admin/users/edit/{}?error={}", user_id, error_msg);
        return Ok(Redirect::to(&redirect_url));
    }

     // Chama o serviço para atualizar as roles permanentes
     // Passa o slice &form.roles
     let update_roles_result = user_service::set_user_roles(&state.db_pool, &user_id, &form.roles).await;

     if let Err(e) = update_roles_result {
         tracing::error!("Erro ao atualizar roles do user {}: {:?}", user_id, e);
         let error_msg = urlencoding::encode("Erro ao atualizar roles na base de dados.");
         // Redireciona de volta para a PÁGINA DE EDIÇÃO com erro
         let redirect_url = format!("/admin/users/edit/{}?error={}", user_id, error_msg);
         return Ok(Redirect::to(&redirect_url));
     }

    // Se chegou aqui, ambas as atualizações foram bem-sucedidas
    tracing::info!("✅ Dados e roles atualizados com sucesso para user {}", user_id);
    let success_msg = urlencoding::encode(&format!("Dados do utilizador '{}' atualizados.", user_id)).to_string();
    // Redireciona para a LISTA com mensagem de sucesso
    let redirect_url = format!("/admin/users?success={}", success_msg);
    Ok(Redirect::to(&redirect_url))
}