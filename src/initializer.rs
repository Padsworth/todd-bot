// initializer.rs
use crate::databaser;
use crate::models::{NewQuote, SchlonghouseMember};
use crate::schema;
use std::fs::{OpenOptions};
use std::io::{self, BufRead}
use poise::serenity_prelude as serenity;
use crate::{Context, Error};

