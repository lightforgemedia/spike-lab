import type { ApiError, ApiErrorResponse } from './types'

/**
 * Base error class for Jules API errors
 */
export class JulesError extends Error {
  readonly code: number
  readonly status: string
  readonly details?: ApiError['details']

  constructor(error: ApiError) {
    super(error.message)
    this.name = 'JulesError'
    this.code = error.code
    this.status = error.status
    this.details = error.details
  }

  static fromResponse(response: ApiErrorResponse): JulesError {
    const { error } = response

    switch (error.code) {
      case 401:
        return new AuthenticationError(error)
      case 403:
        return new ForbiddenError(error)
      case 404:
        return new NotFoundError(error)
      case 429:
        return new RateLimitError(error)
      default:
        return new JulesError(error)
    }
  }
}

/**
 * Authentication failed - invalid or missing API key
 */
export class AuthenticationError extends JulesError {
  constructor(error: ApiError) {
    super(error)
    this.name = 'AuthenticationError'
  }
}

/**
 * Access forbidden - insufficient permissions
 */
export class ForbiddenError extends JulesError {
  constructor(error: ApiError) {
    super(error)
    this.name = 'ForbiddenError'
  }
}

/**
 * Resource not found
 */
export class NotFoundError extends JulesError {
  constructor(error: ApiError) {
    super(error)
    this.name = 'NotFoundError'
  }
}

/**
 * Rate limit exceeded
 */
export class RateLimitError extends JulesError {
  constructor(error: ApiError) {
    super(error)
    this.name = 'RateLimitError'
  }
}
