package task

import "fmt"

type State string

const (
	Planned  State = "planned"
	Assigned State = "assigned"
	Running  State = "running"
	Blocked  State = "blocked"
	Done     State = "done"
	Verified State = "verified"
	Merged   State = "merged"
)

func CanTransition(from State, to State) bool {
	if from == to {
		return true
	}
	allowed := map[State][]State{
		Planned:  {Assigned, Blocked},
		Assigned: {Running, Blocked},
		Running:  {Done, Blocked},
		Blocked:  {Assigned, Running},
		Done:     {Verified, Blocked},
		Verified: {Merged, Blocked},
	}
	for _, candidate := range allowed[from] {
		if candidate == to {
			return true
		}
	}
	return false
}

func ValidateTransition(from State, to State) error {
	if CanTransition(from, to) {
		return nil
	}
	return fmt.Errorf("invalid task transition %q -> %q", from, to)
}
