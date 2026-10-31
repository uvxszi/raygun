type CommandResult<T> = { status: 'ok'; data: T } | { status: 'error'; error: string };

export const unwrapResult = async <T>(pending: Promise<CommandResult<T>>): Promise<T> => {
  const result = await pending;
  if (result.status === 'error') {
    throw new Error(result.error);
  }
  return result.data;
};
