package dev.codenoesis.fixture

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonPrimitive

data class AccountDto(val id: String, val nickname: String)

fun decodeAccount(payload: JsonObject): AccountDto {
    val id = payload.getValue("id").jsonPrimitive.content
    val nickname = payload.getValue("nickname").jsonPrimitive.content
    return AccountDto(id, nickname)
}

suspend fun getAccount(id: String): AccountDto =
    decodeAccount(httpGet("/accounts/$id"))
