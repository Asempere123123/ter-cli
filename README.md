# Ter CLI
cosas
## Install
(decir dependencias, creo que son cargo generate, cargo binutils y el llvm tools de rustup)
(llvm-tools ya no, pq se instala solo? creo puede ser o algo asi creo haber hecho potencialmente)

A simple installation command is provided
```bash
curl -sSf https://raw.githubusercontent.com/Asempere123123/ter-cli/refs/heads/main/install.sh | sh
```

# ter.toml definitions
| Field | Type | Description | Fill Zone / Example |
| :--- | :--- | :--- | :--- |
| **Project Metadata** | | | |
| `project_name` | `String` | The unique name of the project | "Ter ECU" |
| `chip_name` | `String` | The specific MCU | "STM32F405RG" |
| **Paths & Build** | | | |
| `bin_path` | `String` | Path to the file that will be flashed | "./build/ecu.bin" |
| `build_command` | `String` | The shell command used to compile the project | "make" |
| **Debugging Config** | | | |
| `elf_path` | `Option<PathBuf>` | If set defmt debugging will be attached using the specified elf file | "./target/**/ecu" |
| `string_rtt` | `Option<bool>` | Whether to enable raw RTT (String based) logging. | true |
| **Hardware Config** | | | |
| `hse` | `Option<String>` | Enables the HSE for the bootloader | "25000000" |
| `flash_size` | `Option<u64>` | Flash size allocated to the bootloader (default 16k) | "32" |
| **CAN flashing** | | | |
| `can` | `Option<String>` | Name of the CAN peripheral | "CAN1" |
| `can_tx` | `Option<String>` | CAN tx pin | "PA12" |
| `can_rx` | `Option<String>` | CAN rx pin | "PA11" |
| `can_baudrate` | `Option<String>` | CAN baudrate | "1000000" |
| **CAN slave** | | | |
| `can2` | `Option<String>` | If the can instance used for flashing is a slave peripheral, specify the master as can and the actual instance as can2 | "CAN2" |
| `can2_tx` | `Option<String>` | CAN2 tx pin | "PB13" |
| `can2_rx` | `Option<String>` | CAN2 rx pin | "PB12" |
| **CAN interrupts** | | | |
| `can_tx_int_name` | `Option<String>` | In some specific chips, the interrupt name isnt predictable, this options let you override the default one. https://docs.embassy.dev/embassy-stm32/0.6.0/stm32f103c8/interrupt/typelevel/index.html | "USB_HP_CAN1_TX" |
| `can_rx0_int_name` | `Option<String>` | Same as above | "USB_LP_CAN1_RX0" |
| `can_rx1_int_name` | `Option<String>` | Same as above | "CAN1_RX1" |
| `can_sce_int_name` | `Option<String>` | Same as above | "CAN1_SCE" |
