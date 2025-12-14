// src/templates.rs
use askama::Template; // Trait necessário para Askama
use crate::models::{
    presence::{PresencePerson, PresenceStats}, // <-- Adicionado
    user::User,
};

// Struct para o template `login.html` (external file in templates/ folder)
#[derive(Template)] // Deriva a funcionalidade de template
#[template(path = "login.html")]
pub struct LoginPage {
    // Campo opcional para passar uma mensagem de erro para o template
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "user_page.html")]
pub struct UserPage {
    pub user_id: String,
    pub user_name: String,
    pub is_admin: bool,
    pub can_access_presence: bool,
    // Adicionaremos mais dados do utilizador aqui conforme necessário
}

// <<< ADICIONADO: Struct wrapper para User + Roles >>>
// Necessário porque a struct User base não tem as roles diretamente nela
#[derive(Clone, Debug)] // Não deriva Template
pub struct UserWithRoles {
    pub id: String,
    pub name: String,
    pub turma: String,
    pub ano: i64,
    pub curso: String,
    pub genero: String,
    pub roles: Vec<String>, // Inclui as roles
}

// <<< ADICIONADO: Struct para admin_users.html >>>
#[derive(Template)]
#[template(path = "admin_users.html")]
pub struct AdminUsersPage {
    // Lista de utilizadores com as suas roles
    pub users: Vec<UserWithRoles>,
    // Mensagens de feedback opcionais
    pub success_message: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Template)]
#[template(path = "presence.html")]
pub struct PresencePage<'a> { // Usar lifetime se passar referências
    pub turma_selecionada: i64, // O número da turma (ano) que está sendo exibida
    pub pessoas: &'a [PresencePerson], // Slice da lista de pessoas para a turma
    pub stats: &'a PresenceStats, // Referência para as estatísticas calculadas
    // Não precisamos passar a função de formatação, pois fizemos isso no template agora
}

#[derive(Template)]
#[template(path = "admin_edit_user.html")]
pub struct AdminEditUserPage<'a> { // Usa lifetimes para referências
    // Dados do utilizador a ser editado
    pub user: Option<&'a User>,
    // Lista das roles permanentes que este utilizador possui atualmente
    pub current_user_roles: &'a [String], // Slice de Strings
    // Lista de TODAS as roles permanentes possíveis que podem ser atribuídas
    pub all_defined_roles: &'a [&'static str], // Slice de strings estáticas (da constante)
    // Mensagem de erro opcional (ex: se o GET falhar ao buscar o user)
    pub error_message: Option<String>,
}

impl<'a> AdminEditUserPage<'a> {
    /// Verifica se o utilizador atual possui uma role específica
    pub fn has_role(&self, role: &str) -> bool {
        self.current_user_roles
            .iter()
            .any(|r| r.eq_ignore_ascii_case(role))
    }
}