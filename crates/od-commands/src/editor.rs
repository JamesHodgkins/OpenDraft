use std::sync::mpsc::{channel, Receiver, Sender};

use od_core::{overkill, Document, EntityId, MText, Point};

use crate::prompt::{
    Answer, Cancelled, Hint, NumberOrPoint, Options, Picked, Prompt, Reply, Wants,
};
use crate::{Session, Transaction};

/// What a running command sends the UI thread.
pub(crate) enum Request {
    Ask(Prompt),
    /// Work on the drawing, run on the UI thread between frames.
    Run(Box<dyn FnOnce(&mut Session) + Send>),
    /// A line of feedback for the command line.
    Say(String),
    /// Add objects to the selection, or remove them.
    Select {
        ids: Vec<EntityId>,
        remove: bool,
    },
    /// The command returned.
    Done(Result<(), Cancelled>),
}

/// What a running command is handed to talk to the user and to edit the
/// drawing. Every call that needs an answer blocks until the user gives one;
/// Esc makes it return `Err(Cancelled)`, which a command passes up with `?`.
///
/// ```
/// use od_commands::{Cancelled, Editor, PointOptions};
///
/// fn move_objects(ed: &mut Editor) -> Result<(), Cancelled> {
///     let ids = ed.pickset_or_select("Select objects")?;
///     let base = ed.get_point("Specify base point")?;
///     let to = ed.get_point(PointOptions::new("Specify second point").base(base))?;
///     let (dx, dy) = (to.x - base.x, to.y - base.y);
///     ed.transaction("Move", move |tx| {
///         for id in ids {
///             tx.modify(id, |entity| entity.translated(dx, dy));
///         }
///         Ok(())
///     })
/// }
/// ```
pub struct Editor {
    requests: Sender<Request>,
    replies: Receiver<Reply>,
    pickset: Vec<EntityId>,
}

impl Editor {
    pub(crate) fn new(
        requests: Sender<Request>,
        replies: Receiver<Reply>,
        pickset: Vec<EntityId>,
    ) -> Self {
        Self {
            requests,
            replies,
            pickset,
        }
    }

    pub(crate) fn finish(&self, result: Result<(), Cancelled>) {
        let _ = self.requests.send(Request::Done(result));
    }

    /// The objects that were selected when the command started.
    pub fn pickset(&self) -> &[EntityId] {
        &self.pickset
    }

    /// The preselected objects, or else ask the user to select some. Fails
    /// with `Cancelled` if the user selects nothing.
    pub fn pickset_or_select(&mut self, message: &str) -> Result<Vec<EntityId>, Cancelled> {
        if !self.pickset.is_empty() {
            return Ok(self.pickset.clone());
        }
        self.get_selection(message)
    }

    /// Show a line of feedback in the command line.
    pub fn say(&self, text: impl Into<String>) {
        let _ = self.requests.send(Request::Say(text.into()));
    }

    /// Add objects to the selection.
    pub fn select(&self, ids: Vec<EntityId>) {
        let _ = self.requests.send(Request::Select { ids, remove: false });
    }

    /// Remove objects from the selection.
    pub fn deselect(&self, ids: Vec<EntityId>) {
        let _ = self.requests.send(Request::Select { ids, remove: true });
    }

    /// Read the drawing. The closure runs on the UI thread, so it cannot
    /// borrow from the command; return what you need to keep.
    pub fn read<R: Send + 'static>(
        &self,
        read: impl FnOnce(&Document) -> R + Send + 'static,
    ) -> Result<R, Cancelled> {
        self.run(move |session| read(&session.document))
    }

    /// Like [`Self::read`], and also hands over the revision the drawing is
    /// at, so what the closure works out belongs to exactly that revision.
    pub fn read_revision<R: Send + 'static>(
        &self,
        read: impl FnOnce(&Document, u64) -> R + Send + 'static,
    ) -> Result<R, Cancelled> {
        self.run(move |session| read(&session.document, session.revision()))
    }

    /// Edit the drawing as one undo step named `name`. If the closure returns
    /// an error, every edit it made is reverted and nothing is recorded.
    pub fn transaction<T: Send + 'static>(
        &self,
        name: &'static str,
        edit: impl FnOnce(&mut Transaction) -> Result<T, Cancelled> + Send + 'static,
    ) -> Result<T, Cancelled> {
        self.run(move |session| session.transaction(name, edit))?
    }

    /// Undo the most recent step in the drawing's history. A command uses
    /// this only to take back a step it made itself (Line's Undo option).
    pub fn undo(&self) -> Result<(), Cancelled> {
        self.run(|session| {
            session.undo();
        })
    }

    fn run<R: Send + 'static>(
        &self,
        job: impl FnOnce(&mut Session) -> R + Send + 'static,
    ) -> Result<R, Cancelled> {
        let (done, result) = channel();
        let job = Box::new(move |session: &mut Session| {
            let _ = done.send(job(session));
        });
        self.requests
            .send(Request::Run(job))
            .map_err(|_| Cancelled)?;
        result.recv().map_err(|_| Cancelled)
    }

    fn ask(&self, prompt: Prompt) -> Result<Reply, Cancelled> {
        self.requests
            .send(Request::Ask(prompt))
            .map_err(|_| Cancelled)?;
        match self.replies.recv() {
            Ok(Reply::Cancel) | Err(_) => Err(Cancelled),
            Ok(reply) => Ok(reply),
        }
    }

    /// Ask for a value of one kind. A reply of another kind (a UI bug) is
    /// treated as a cancel rather than guessed at.
    fn ask_value<T: Clone>(
        &self,
        wants: Wants,
        options: &Options<T>,
        hint: impl Fn(&T) -> Hint,
        extract: impl Fn(Reply) -> Option<T>,
    ) -> Result<Answer<T>, Cancelled> {
        let prompt = Prompt::new(wants, options, options.default.as_ref().map(hint));
        match self.ask(prompt)? {
            Reply::Enter => Ok(match &options.default {
                Some(value) => Answer::Value(value.clone()),
                None => Answer::Enter,
            }),
            Reply::Keyword(label) if options.keywords.iter().any(|k| k.label == label) => {
                Ok(Answer::Keyword(label))
            }
            reply => extract(reply).map(Answer::Value).ok_or(Cancelled),
        }
    }

    /// Ask for a point. A keyword or a bare Enter comes back as such.
    pub fn ask_point(
        &self,
        options: impl Into<Options<Point>>,
    ) -> Result<Answer<Point>, Cancelled> {
        self.ask_value(
            Wants::Point,
            &options.into(),
            |p| Hint::Point(*p),
            |r| match r {
                Reply::Point(p) => Some(p),
                _ => None,
            },
        )
    }

    pub fn ask_distance(&self, options: impl Into<Options<f64>>) -> Result<Answer<f64>, Cancelled> {
        self.ask_number_of(Wants::Distance, options.into())
    }

    /// An angle in radians, counter-clockwise.
    pub fn ask_angle(&self, options: impl Into<Options<f64>>) -> Result<Answer<f64>, Cancelled> {
        self.ask_number_of(Wants::Angle, options.into())
    }

    /// An angle typed, or a point to start measuring one from (ask for the
    /// second point yourself).
    pub fn ask_angle_or_point(
        &self,
        options: impl Into<Options<f64>>,
    ) -> Result<Answer<NumberOrPoint>, Cancelled> {
        self.ask_number_or_point(Wants::AngleOrPoint, options.into())
    }

    /// A length typed, or a point to start measuring one from.
    pub fn ask_distance_or_point(
        &self,
        options: impl Into<Options<f64>>,
    ) -> Result<Answer<NumberOrPoint>, Cancelled> {
        self.ask_number_or_point(Wants::DistanceOrPoint, options.into())
    }

    fn ask_number_or_point(
        &self,
        wants: Wants,
        options: Options<f64>,
    ) -> Result<Answer<NumberOrPoint>, Cancelled> {
        let prompt = Prompt::new(wants, &options, options.default.map(Hint::Number));
        match self.ask(prompt)? {
            Reply::Enter => Ok(match options.default {
                Some(value) => Answer::Value(NumberOrPoint::Number(value)),
                None => Answer::Enter,
            }),
            Reply::Keyword(label) if options.keywords.iter().any(|k| k.label == label) => {
                Ok(Answer::Keyword(label))
            }
            Reply::Number(n) => Ok(Answer::Value(NumberOrPoint::Number(n))),
            Reply::Point(p) => Ok(Answer::Value(NumberOrPoint::Point(p))),
            _ => Err(Cancelled),
        }
    }

    pub fn ask_number(&self, options: impl Into<Options<f64>>) -> Result<Answer<f64>, Cancelled> {
        self.ask_number_of(Wants::Number, options.into())
    }

    fn ask_number_of(&self, wants: Wants, options: Options<f64>) -> Result<Answer<f64>, Cancelled> {
        self.ask_value(
            wants,
            &options,
            |n| Hint::Number(*n),
            |r| match r {
                Reply::Number(n) => Some(n),
                _ => None,
            },
        )
    }

    pub fn ask_string(
        &self,
        options: impl Into<Options<String>>,
    ) -> Result<Answer<String>, Cancelled> {
        self.ask_value(
            Wants::Text,
            &options.into(),
            |s| Hint::Text(s.clone()),
            |r| match r {
                Reply::Text(s) => Some(s),
                _ => None,
            },
        )
    }

    /// Choose one of `options.keywords`; returns its label.
    pub fn get_keyword(&self, options: impl Into<Options<String>>) -> Result<String, Cancelled> {
        let options = options.into();
        let prompt = Prompt::new(
            Wants::Keyword,
            &options,
            options.default.clone().map(Hint::Text),
        );
        match self.ask(prompt)? {
            Reply::Keyword(label) if options.keywords.iter().any(|k| k.label == label) => Ok(label),
            Reply::Enter => options.default.ok_or(Cancelled),
            _ => Err(Cancelled),
        }
    }

    /// Choose one of `options.keywords`, typed or clicked. Enter alone is the
    /// default's label if there is one, else `None`: an option menu that
    /// ends when the user presses Enter.
    pub fn ask_keyword(
        &self,
        options: impl Into<Options<String>>,
    ) -> Result<Option<String>, Cancelled> {
        let options = options.into();
        let prompt = Prompt::new(
            Wants::Keyword,
            &options,
            options.default.clone().map(Hint::Text),
        );
        match self.ask(prompt)? {
            Reply::Keyword(label) if options.keywords.iter().any(|k| k.label == label) => {
                Ok(Some(label))
            }
            Reply::Enter => Ok(options.default),
            _ => Err(Cancelled),
        }
    }

    /// Ask the user to select objects. Enter accepts the current selection.
    pub fn get_selection(&self, message: &str) -> Result<Vec<EntityId>, Cancelled> {
        let options = Options::<()>::new(message);
        match self.ask(Prompt::new(Wants::Selection, &options, None))? {
            Reply::Selection(ids) if !ids.is_empty() => Ok(ids),
            _ => Err(Cancelled),
        }
    }

    /// Ask the user to select objects, like `get_selection`, but an empty
    /// answer is allowed when `options` says so (`Options::allow_empty`).
    pub fn ask_selection(
        &self,
        options: impl Into<Options<()>>,
    ) -> Result<Vec<EntityId>, Cancelled> {
        let options = options.into();
        match self.ask(Prompt::new(Wants::Selection, &options, None))? {
            Reply::Selection(ids) if !ids.is_empty() || options.allow_empty => Ok(ids),
            _ => Err(Cancelled),
        }
    }

    /// Ask the user to click one object. The answer is the object, the
    /// point that was clicked on it and whether Shift was held; keywords and a bare Enter come back as
    /// such.
    pub fn ask_object(
        &self,
        options: impl Into<Options<Picked>>,
    ) -> Result<Answer<Picked>, Cancelled> {
        self.ask_value(
            Wants::Object,
            &options.into(),
            |_| Hint::Text(String::new()),
            |r| match r {
                Reply::Object { id, at, shift } => Some(Picked { id, at, shift }),
                _ => None,
            },
        )
    }

    /// Open the in-place text editor on `text` and wait. `Some` is the text
    /// as it was accepted; `None` is the editor closed with nothing to keep.
    /// Esc is `Err(Cancelled)`, like any prompt.
    pub fn edit_mtext(&self, text: MText) -> Result<Option<MText>, Cancelled> {
        self.edit_mtext_of(None, text)
    }

    /// [`Self::edit_mtext`] on the text object `id`, which the drawing then
    /// leaves out while the editor shows its draft.
    pub fn edit_mtext_entity(&self, id: EntityId, text: MText) -> Result<Option<MText>, Cancelled> {
        self.edit_mtext_of(Some(id), text)
    }

    fn edit_mtext_of(&self, of: Option<EntityId>, text: MText) -> Result<Option<MText>, Cancelled> {
        let options = Options::<()>::new("Edit text");
        let mut prompt = Prompt::new(Wants::MTextEditor, &options, None);
        prompt.mtext = Some(Box::new(text));
        prompt.mtext_of = of;
        match self.ask(prompt)? {
            Reply::MText(text) => Ok(Some(*text)),
            Reply::Enter => Ok(None),
            _ => Err(Cancelled),
        }
    }

    /// Open the OVERKILL dialog on `settings` and wait for OK. Cancel is
    /// `Err(Cancelled)`.
    pub fn ask_overkill(
        &self,
        settings: overkill::Settings,
    ) -> Result<overkill::Settings, Cancelled> {
        let options = Options::<()>::new("Object Comparison Settings");
        let mut prompt = Prompt::new(Wants::OverkillDialog, &options, None);
        prompt.overkill = Some(settings);
        match self.ask(prompt)? {
            Reply::Overkill(settings) => Ok(settings),
            _ => Err(Cancelled),
        }
    }

    /// Like `ask_point`, for commands with no keywords: Enter with no default
    /// ends the command.
    pub fn get_point(&self, options: impl Into<Options<Point>>) -> Result<Point, Cancelled> {
        self.value(self.ask_point(options)?)
    }

    pub fn get_distance(&self, options: impl Into<Options<f64>>) -> Result<f64, Cancelled> {
        self.value(self.ask_distance(options)?)
    }

    /// An angle in radians, counter-clockwise.
    pub fn get_angle(&self, options: impl Into<Options<f64>>) -> Result<f64, Cancelled> {
        self.value(self.ask_angle(options)?)
    }

    pub fn get_number(&self, options: impl Into<Options<f64>>) -> Result<f64, Cancelled> {
        self.value(self.ask_number(options)?)
    }

    pub fn get_string(&self, options: impl Into<Options<String>>) -> Result<String, Cancelled> {
        self.value(self.ask_string(options)?)
    }

    fn value<T>(&self, answer: Answer<T>) -> Result<T, Cancelled> {
        match answer {
            Answer::Value(value) => Ok(value),
            Answer::Keyword(_) | Answer::Enter => Err(Cancelled),
        }
    }
}
