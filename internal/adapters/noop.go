package adapters

import "context"

type NoopGraph struct{}

func (NoopGraph) Available(context.Context) bool { return false }
func (NoopGraph) Backlinks(context.Context, string) ([]string, error) {
	return nil, nil
}

type NoopMemory struct{}

func (NoopMemory) Save(context.Context, string, string) error { return nil }
func (NoopMemory) Search(context.Context, string) ([]string, error) {
	return nil, nil
}
