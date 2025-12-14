// src/templates.rs
use askama::Template;
use crate::models::{
    presence::{PresencePerson, PresenceStats}, // Necessário para PresencePage
    user::User, // Necessário para AdminEditUserPage
};

// --- LOGIN ---

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginPage {
    pub error: Option<String>,
}

// --- DASHBOARD (USER) ---

#[derive(Debug, Clone)]
pub struct MeuServico {
    pub data: String,
    pub dia_semana: String,
    pub dia_mes: String,
    pub mes_extenso: String,
    pub posto: String,
}

#[derive(Debug, Clone)]
pub struct NotificacaoTroca {
    pub troca_id: String,
    pub solicitante: String,
    pub data: String,
    pub posto: String,
    pub motivo: String,
}

#[derive(Template)]
#[template(path = "user_page.html")]
pub struct UserPage {
    pub user_id: String,
    pub name: String,
    pub meus_servicos: Vec<MeuServico>,
    pub trocas_pendentes: Vec<NotificacaoTroca>,
}

// --- ESCALAS ---

#[derive(Debug, Clone)]
pub struct AlocacaoExibicao {
    pub alocacao_id: String,
    pub user_id: String,
    pub posto: String,
    pub militar: String,
    pub turma: String,
    pub is_punicao: bool,
    pub is_meu: bool,
}

#[derive(Debug, Clone)]
pub struct EscalaDiaView {
    pub data: String,
    pub data_formatada: String,
    pub tipo: String,
    pub status: String,
    pub alocacoes: Vec<AlocacaoExibicao>,
}

#[derive(Template)]
#[template(path = "escala.html")]
pub struct EscalaTemplate {
    pub dias_publicados: Vec<EscalaDiaView>,
    pub dias_rascunho: Vec<EscalaDiaView>,
    pub is_admin: bool,
    pub user_atual_id: String,
}

// --- PRESENÇA ---

#[derive(Template)]
#[template(path = "presence.html")]
pub struct PresencePage<'a> {
    pub turma_selecionada: i64,
    pub pessoas: &'a [PresencePerson],
    pub stats: &'a PresenceStats,
}

// --- ADMINISTRAÇÃO DE UTILIZADORES ---

#[derive(Clone, Debug)]
pub struct UserWithRoles {
    pub id: String,
    pub name: String,
    pub turma: String,
    pub ano: i64,
    pub curso: String,
    pub genero: String,
    pub roles: Vec<String>,
}

#[derive(Template)]
#[template(path = "admin_users.html")]
pub struct AdminUsersPage {
    pub users: Vec<UserWithRoles>,
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "admin_edit_user.html")]
pub struct AdminEditUserPage<'a> {
    pub user: Option<&'a User>,
    pub current_user_roles: &'a [String],
    pub all_defined_roles: &'a [&'static str],
    pub error_message: Option<String>,
}

impl<'a> AdminEditUserPage<'a> {
    pub fn has_role(&self, role: &str) -> bool {
        self.current_user_roles
            .iter()
            .any(|r| r.eq_ignore_ascii_case(role))
    }
}

#[derive(Template)]
#[template(path = "admin_escala.html")]
pub struct AdminEscalaPage {
    pub user_name: String,
    // Podemos adicionar estatísticas aqui no futuro (ex: "X dias rascunho")
}