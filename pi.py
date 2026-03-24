from decimal import Decimal, getcontext

def chudnovsky_algorithm(precision):
    """Calculates π to the specified precision using the Chudnovsky algorithm."""
    getcontext().prec = precision + 2
    
    C = 426880 * Decimal(10005).sqrt()
    K = 6
    M = 1
    X = 1
    L = 13591409
    S = L

    # Number of iterations needed for the given precision
    # Each iteration adds approximately 14.18 digits
    iterations = (precision // 14) + 1

    for k in range(1, iterations):
        M = (K**3 - 16*K) * M // k**3
        L += 545140134
        X *= -262537412640768000
        S += Decimal(M * L) / X
        K += 12

    pi = C / S
    return +pi

if __name__ == "__main__":
    PRECISION = 20000
    print(f"Calculating Pi to {PRECISION} digits using Chudnovsky algorithm...")
    pi_val = chudnovsky_algorithm(PRECISION)
    
    # Analysis
    pi_str = str(pi_val).replace(".", "")
    # Extract only the decimal places (skip the '3')
    decimal_digits = pi_str[1:PRECISION+1]
    
    print(f"\nDigit Frequency Analysis (First {PRECISION} decimal places):")
    print("-" * 45)
    
    counts = {str(i): decimal_digits.count(str(i)) for i in range(10)}
    
    for digit in sorted(counts.keys()):
        count = counts[digit]
        percentage = (count / len(decimal_digits)) * 100
        # Create a simple visual histogram bar
        bar = "█" * int(percentage * 2)
        print(f"Digit {digit}: {count:5} ({percentage:5.2f}%) {bar}")
    
    print("-" * 45)
    print(f"Total digits analyzed: {len(decimal_digits)}")