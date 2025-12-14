// src/web/routes.rs
use crate::{
    state::AppState,
    // Adicionar presence_handlers
    web::{admin_handlers, auth_handlers, mw_auth, mw_admin, mw_presence, presence_handlers, user_handlers, escala_handlers},
};
use axum::{
    middleware,
    routing::{get, post},
    Router,
};

pub fn create_router(app_state: AppState) -> Router {

    // --- Rotas Públicas --- (Mantido igual)
    let public_routes = Router::new()
        .route("/login", get(auth_handlers::show_login_form).post(auth_handlers::handle_login))
        .route("/logout", get(auth_handlers::handle_logout))
        .route("/", get(|| async { axum::response::Redirect::permanent("/login") }));

    // --- Rotas de Admin --- (Mantido igual)
    // Exigem login E role admin
    let admin_routes = Router::new()
        .route("/users", get(admin_handlers::show_admin_users_page))
        .route("/users/create", post(admin_handlers::handle_create_user))
        .route("/users/change_password", post(admin_handlers::handle_change_password))
        .route("/users/edit/{id}", // <-- MUDANÇA AQUI
            get(admin_handlers::show_edit_user_form)
            .post(admin_handlers::handle_edit_user)
        )
        // Aplica APENAS mw_admin aqui (mw_auth será aplicado no router pai)
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            mw_admin::require_admin,
        ));

    // *** ALTERADO: Criar router específico para Presença ***
    let presence_routes = Router::new()
        .route("/", get(presence_handlers::presence_page_handler)) // Rota base é /presence
        .route("/ws", get(presence_handlers::presence_websocket_handler)) // Rota é /presence/ws
        // Aplica APENAS mw_presence aqui (mw_auth será aplicado no router pai)
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            mw_presence::require_presence_access,
        ));

    let escala_routes = Router::new()
        // Gera a escala (JSON: { "data": "2025-10-25", "tipo": "RN" })
        .route("/gerar", post(escala_handlers::handle_gerar_escala))
        .route("/", get(escala_handlers::handle_pagina_escala))
        // Aprova troca (URL: /escala/trocas/{id}/aprovar)
        .route("/trocas/{id}/aprovar", post(escala_handlers::handle_aprovar_troca))
        // Vê a escala (URL: /escala/ver?data=2025-10-25)
        .route("/ver", get(escala_handlers::handle_ver_escala));
        // Aqui você pode adicionar um middleware de Admin se quiser proteger estas ações
        // .route_layer(middleware::from_fn_with_state(app_state.clone(), mw_admin::require_admin));


    // --- Rotas Autenticadas (Combinando tudo) ---
    // Exigem *pelo menos* login
    let authenticated_routes = Router::new()
        // Rotas que exigem apenas login
        .route("/user", get(user_handlers::user_page_handler))
        // Adicionar outras rotas autenticadas gerais aqui...

        // Aninha as rotas de admin sob /admin
        .nest("/admin", admin_routes)
        .nest("/escala", escala_routes)
        // *** ALTERADO: Aninha as rotas de presença sob /presence ***
        .nest("/presence", presence_routes)

        // Aplica o middleware geral require_auth a TODAS as rotas
        // definidas ACIMA neste router (incluindo as aninhadas /admin/* e /presence/*)
        .route_layer(middleware::from_fn_with_state(
            app_state.clone(),
            mw_auth::require_auth,
        ));

    // --- Router Final --- (Mantido igual)
    Router::new()
        .merge(public_routes)
        .merge(authenticated_routes)
        .with_state(app_state)
}